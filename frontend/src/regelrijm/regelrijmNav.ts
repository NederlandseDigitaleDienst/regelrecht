// Navigation for the RegelRijm pane: where a name in a rule comes from, and
// whether it leads to another rule. Pure functions, so the pane stays thin and
// this can be tested without a browser.
import { normaliseAction, writtenOf } from '@regelrecht/frontend-shared/regelrijm/index';

/** A plain object from a tree that may be half-way through an edit. */
type Obj = Record<string, unknown>;

// A list from a tree that may be half-way through an edit: anything that is
// not a list counts as empty, and entries that are no object are skipped.
const listOf = (v: unknown): Obj[] => (Array.isArray(v) ? v.filter((x): x is Obj => x !== null && typeof x === 'object') : []);

// A field of an object, or undefined when `v` is no object.
const field = (v: unknown, key: string): unknown => (v !== null && typeof v === 'object' ? (v as Obj)[key] : undefined);

/** An action of a law, with the article it belongs to. */
export interface ActionEntry {
  article: Obj;
  action: Obj;
}

/** Where a reference gets its value (see resolveReference). */
export type ReferenceTarget =
  | { kind: 'action'; name: string; lawId: string | null; articleNumber: string; output: string }
  | { kind: 'external'; name: string; lawId: string; output: string; description?: unknown }
  | { kind: 'data'; name: string; description?: unknown }
  | { kind: 'parameter'; name: string; description?: unknown }
  | { kind: 'definition'; name: string; value: unknown }
  | { kind: 'context'; name: string }
  | { kind: 'local'; name: string };

/** A reference that leads to another rule. */
export type NavigableTarget = Extract<ReferenceTarget, { kind: 'action' | 'external' }>;

/** Where following a reference leads (see followReference). */
export type FollowResult =
  | { kind: 'same-article'; output: string }
  | { kind: 'navigate'; lawId: string; articleNumber: string }
  | { kind: 'error'; message: string };

/** Every action of a law, with the article it belongs to. */
export function actionsOf(law: unknown): ActionEntry[] {
  const out: ActionEntry[] = [];
  for (const article of listOf(field(law, 'articles'))) {
    for (const action of listOf(field(field(field(article, 'machine_readable'), 'execution'), 'actions'))) {
      out.push({ article, action });
    }
  }
  return out;
}

/** The article and action that compute `output` in `law`, or null. */
export function findOutput(law: unknown, output: unknown): ActionEntry | null {
  return actionsOf(law).find(({ action }) => action.output === output) ?? null;
}

/**
 * Where a reference in a rule of `article` (in `law`) gets its value.
 *
 * kind: 'action'    — computed by an action; `lawId`, `articleNumber`, `output`
 *       'external'  — an input from another law; `lawId`, `output`
 *       'data'      — an input from outside the corpus
 *       'parameter' — passed in by whoever runs the law
 *       'definition'— a constant of the article; `value`
 *       'context'   — set by the engine (the calculation date)
 *       'local'     — a FOREACH variable or a field of one
 */
export function resolveReference(ref: string, law: unknown, article: unknown): ReferenceTarget {
  // split() always gives at least one element.
  const base = ref.slice(1).split('.')[0]!;
  const mr = field(article, 'machine_readable') ?? {};
  const ex = field(mr, 'execution') ?? {};
  const $id = field(law, '$id');
  const lawId = typeof $id === 'string' ? $id : null;
  if (base === 'referencedate') return { kind: 'context', name: base };
  if (listOf(field(ex, 'actions')).some((a) => a.output === base)) {
    return { kind: 'action', name: base, lawId, articleNumber: String(field(article, 'number')), output: base };
  }
  const input = listOf(field(ex, 'input')).find((i) => i.name === base);
  if (input) {
    const src: Obj = input.source && typeof input.source === 'object' ? (input.source as Obj) : {};
    if (src.regulation) return { kind: 'external', name: base, lawId: String(src.regulation), output: typeof src.output === 'string' ? src.output : base, description: input.description };
    if (typeof src.output === 'string' && src.output) {
      const hit = findOutput(law, src.output);
      if (hit) return { kind: 'action', name: base, lawId, articleNumber: String(hit.article.number), output: src.output };
    }
    return { kind: 'data', name: base, description: src.description ?? input.description };
  }
  const param = listOf(field(ex, 'parameters')).find((p) => p.name === base);
  if (param) return { kind: 'parameter', name: base, description: param.description };
  const definitions = field(mr, 'definitions');
  if (definitions && typeof definitions === 'object' && !Array.isArray(definitions) && Object.hasOwn(definitions, base)) {
    const def = (definitions as Obj)[base];
    const value = def && typeof def === 'object' && !Array.isArray(def) && 'value' in def ? def.value : def;
    return { kind: 'definition', name: base, value };
  }
  const elsewhere = findOutput(law, base);
  if (elsewhere) return { kind: 'action', name: base, lawId, articleNumber: String(elsewhere.article.number), output: base };
  return { kind: 'local', name: base };
}

/** Whether following a reference of this kind leads to another rule. */
export function isNavigable(target: { kind: string }): target is NavigableTarget {
  return target.kind === 'action' || target.kind === 'external';
}

export interface OriginHintOptions {
  destination: (target: NavigableTarget) => string;
  format: (target: Extract<ReferenceTarget, { kind: 'definition' }>) => string;
  label?: (name: string) => string;
}

export interface FollowOptions {
  law: unknown;
  lawId: string | null;
  article: unknown;
  load: (lawId: string) => Promise<{ law?: unknown } | null | undefined>;
  label?: (name: string) => string;
}

/**
 * The tooltip of a name in a reading: where its value comes from, or
 * undefined when there is nothing to say. `destination(target)` names the
 * rule a computed name leads to; `format(target)` writes a constant's value.
 * A computed name is a token, which a narrow pane may cut short, so its
 * tooltip names it in full.
 */
export function originHint(ref: string, target: ReferenceTarget, { destination, format, label = (s) => s }: OriginHintOptions): string | undefined {
  // split() always gives at least one element.
  if (isNavigable(target)) return `${label(ref.slice(1).split('.')[0]!)}: berekend in ${destination(target)}`;
  if (target.kind === 'definition') return `Vaste waarde in dit artikel: ${format(target)}`;
  if (target.kind === 'parameter') return 'Wordt meegegeven bij het uitvoeren';
  if (target.kind === 'context') return 'De rekendatum';
  if (target.kind === 'data') return target.description ? `Gegeven van buiten: ${target.description}` : 'Gegeven van buiten';
  return undefined;
}

/**
 * Where following `ref` leads, for a click on a name in a reading:
 * `{ kind: 'same-article', output }`, `{ kind: 'navigate', lawId, articleNumber }`,
 * `{ kind: 'error', message }` or null when the name leads nowhere. `load`
 * fetches another regelwerk (`lawId → { law }`).
 */
export async function followReference(ref: string, { law, lawId, article, load, label = (s) => s }: FollowOptions): Promise<FollowResult | null> {
  const t = resolveReference(ref, law, article);
  if (t.kind === 'action') {
    // resolveReference names the law by its `$id`; the editor's route may name
    // it differently, so a rule in this law goes by the route's id.
    const sameLaw = t.lawId === (field(law, '$id') ?? lawId);
    if (sameLaw && String(t.articleNumber) === String(field(article, 'number'))) return { kind: 'same-article', output: t.output };
    // resolveReference only searches `law`, so the rule is in this law: go by
    // the route's id, or the `$id` when the route has none. A half-edited law
    // with neither has no name to navigate to.
    const target = lawId ?? t.lawId;
    if (target === null || target === undefined) return { kind: 'error', message: 'Dit regelwerk heeft nog geen $id, dus de rekenregel is niet te openen.' };
    return { kind: 'navigate', lawId: String(target), articleNumber: String(t.articleNumber) };
  }
  if (t.kind !== 'external') return null;
  let entry;
  try {
    entry = await load(t.lawId);
  } catch (e) {
    return { kind: 'error', message: `${label(t.lawId)} is niet geladen: ${e instanceof Error ? e.message : String(e)}` };
  }
  const hit = findOutput(entry?.law, t.output);
  if (!hit) return { kind: 'error', message: `${label(t.output)} is in ${label(t.lawId)} geen uitkomst van een rekenregel.` };
  return { kind: 'navigate', lawId: t.lawId, articleNumber: String(hit.article.number) };
}

/**
 * The operation of the action as written at `path`, a path in the normalised
 * action of a RegelRijm reading (`value`, `value/values/1`, …). Normalising
 * flattens nested ADD/MULTIPLY/AND/OR, so the path is walked in the normalised
 * action and mapped back through `writtenOf`; undefined where no operation
 * stands.
 */
export function rawAt(action: unknown, path: readonly (string | number)[]): unknown {
  if (!Array.isArray(path) || path[0] !== 'value') return undefined;
  let v = normaliseAction(action).value;
  for (const step of path.slice(1)) {
    if (v === null || typeof v !== 'object') return undefined;
    v = (v as Record<string | number, unknown>)[step];
  }
  return writtenOf(v);
}
