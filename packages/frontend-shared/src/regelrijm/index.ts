// RegelRijm: one unambiguous Dutch reading of an action tree (RFC-054).
//
// Pure functions: the same action, article and glossary give the same bytes.
// The reading is for people who read; modellers write YAML. There is no
// parser here on purpose: the one that proves the reading unambiguous lives
// next to the tests and is not an input channel.
import { group, nest, line, hardline, tok } from './doc';
import { makeVocabulary } from './names';
import { normaliseAction, isOperation } from './normalise';
import { Renderer, LEVEL, finish, chooseLegend } from './render';
import { declaredUnits, literalUnits, pathKey } from './units';
import type { Fold, Rendered } from './render';
import { isRecord, type ArticleContext, type Doc, type FoldInput, type Glossary, type NormalisedAction, type Path, type Segment, type Unit } from './types';

export type * from './types';
export type { Fold } from './render';

/** The full reading of an action. */
export interface ActionReading {
  output: unknown;
  text: string;
  segments: Segment[];
  normalised: NormalisedAction;
  /** The number (1, 1.2, …) of every operation, by path key. */
  numbers: Map<string, string>;
}

/** The reading of one operation inside an action, deeper parts folded. */
export interface FocusReading {
  text: string;
  segments: Segment[];
  folds: Fold[];
  number: string;
  node: unknown;
  signature: string | null;
  isOperation: boolean;
}

export interface ReadingOptions {
  article?: unknown;
  ctx?: ArticleContext | null;
  glossary?: Glossary;
}

export interface FocusOptions extends ReadingOptions {
  depth?: number;
  open?: ReadonlySet<string>;
}

export { OPERATIONS, OPERATION_FIELDS, normalise, normaliseAction, writtenOf } from './normalise';
export { makeVocabulary, humanizeName, BUILTIN_TERMS } from './names';
export { declaredUnits, pathKey } from './units';
export { formatLiteral } from './literals';
export { LEGEND_MIN, ENUMERATION_MIN } from './render';
export { SIGNATURE_MAX_INPUTS } from './signature';

/**
 * Everything the reading needs from outside the tree: the units the article
 * declares and the vocabulary. Build it once per article.
 */
export function articleContext(article: unknown, glossary?: Glossary): ArticleContext {
  const mr = isRecord(article) ? article.machine_readable : undefined;
  const definitions = isRecord(mr) ? mr.definitions : undefined;
  const constants = new Set(isRecord(definitions) ? Object.keys(definitions) : []);
  return { declared: declaredUnits(article), vocab: makeVocabulary(glossary), constants };
}

function outputUnit(output: unknown, declared: ReadonlyMap<string, Unit>): Unit | null {
  return typeof output === 'string' ? (declared.get(output) ?? null) : null;
}

function rootDoc(r: Renderer, normalised: NormalisedAction, rootResult: Rendered): Doc {
  // Mid-edit an action can lack its output; that reads as a hole, not as a name.
  const name = typeof normalised.output === 'string' && normalised.output
    ? r.name(`$${normalised.output}`, ['output']).doc
    : r.hole('output', ['output']).doc;
  return group(name, ' ', tok('keyword', 'wordt:'), nest(line, rootResult.doc));
}

/** `waarbij:` and one line `A = …` per letter, in order of first use. */
function legendDoc(r: Renderer): Doc {
  const entries: Doc[] = [];
  // Reading a definition can name further letters; they join the end.
  for (let i = 0; i < r.legend.length; i++) {
    // `i` stays below `r.legend.length`.
    const { letter, rep } = r.legend[i]!;
    const def = r.definition(rep);
    entries.push(hardline, tok('letter', letter, { path: rep.path, letter }), ' ', tok('keyword', '='), ' ', nest(def.doc));
  }
  return [hardline, tok('keyword', 'waarbij:'), nest(...entries)];
}

function valueOf(normalised: NormalisedAction, path: Path): unknown {
  let v: unknown = normalised;
  for (const step of path) {
    if (v === null || typeof v !== 'object') return undefined;
    v = (v as Record<string, unknown>)[step];
  }
  return v;
}

/**
 * The full reading of one action: `[uitkomst] wordt: …`.
 */
export function renderAction(action: unknown, { article = null, ctx = null, glossary = undefined }: ReadingOptions = {}): ActionReading {
  const c = ctx ?? articleContext(article, glossary);
  const normalised = normaliseAction(action);
  const units = literalUnits(normalised.value, outputUnit(normalised.output, c.declared), c.declared, ['value']);
  const base = { vocab: c.vocab, units, declared: c.declared };
  if (!('value' in normalised)) {
    const r = new Renderer(base);
    const body: Rendered = { doc: tok('error', normalised.resolve ? '⟨wordt opgezocht (resolve)⟩' : '⟨geen waarde⟩'), level: LEVEL.CLOSED };
    const { segments, text } = finish(rootDoc(r, normalised, body));
    return { output: normalised.output, text, segments, normalised, numbers: r.numbers };
  }
  // First the reading in place, which also measures every part; then, if
  // parts repeat, the reading with a legend. Numbers follow the first, so a
  // part has the same number whether or not a letter stands for it.
  const r1 = new Renderer({ ...base, record: true });
  const body1 = r1.node(normalised.value, ['value'], '1', 0);
  // A renderer made with `record: true` always has its record list.
  const { hoist, reps } = chooseLegend(r1.record!);
  let doc: Doc = rootDoc(r1, normalised, body1);
  if (hoist.size) {
    const r2 = new Renderer({ ...base, hoist, reps });
    doc = [rootDoc(r2, normalised, r2.node(normalised.value, ['value'], '1', 0)), legendDoc(r2)];
  }
  const { segments, text } = finish(doc);
  return { output: normalised.output, text, segments, normalised, numbers: r1.numbers };
}

/**
 * The reading of the operation at `path` inside an action, showing `depth`
 * levels of operations (1: only the one at `path`) and folding deeper ones
 * into their signature, `⟨bedrag uit [a] en [b]⟩`. `open` holds the path
 * keys of folded parts to show one level deeper (the reader clicked them).
 * The folded text is a view: the full text stays the reading.
 */
export function renderFocus(
  action: unknown,
  path: Path = ['value'],
  { article = null, ctx = null, glossary = undefined, depth = 2, open = new Set<string>() }: FocusOptions = {},
): FocusReading {
  const c = ctx ?? articleContext(article, glossary);
  const full = renderAction(action, { article, ctx: c });
  const normalised = full.normalised;
  const node = valueOf(normalised, path);
  const opUnits = new Map<string, Unit | null>();
  const units = literalUnits(normalised.value, outputUnit(normalised.output, c.declared), c.declared, ['value'], opUnits);
  const r = new Renderer({ vocab: c.vocab, units, declared: c.declared, opUnits, constants: c.constants ?? new Set(), fold: { depth, open } });
  const number = full.numbers.get(pathKey(path)) ?? '1';
  const res = r.node(node, path, number, 0);
  const doc = path.length === 1 ? rootDoc(r, normalised, res) : res.doc;
  const { segments, text } = finish(doc);
  const signature = isOperation(node) ? r.signature(node, path) : null;
  return { text, segments, folds: r.folds, number, node, signature, isOperation: isOperation(node) };
}

/** The names a reading shows without a term, with how often each occurs. */
export function untranslated(segments: readonly Segment[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const s of segments) {
    if (typeof s !== 'string' && s.tk === 'name' && s.segment !== undefined) counts.set(s.segment, (counts.get(s.segment) ?? 0) + 1);
  }
  return counts;
}

// A fold names the inputs of its signature; they count as references too.
function withFoldInputs(segments: readonly Segment[]): (Segment | FoldInput)[] {
  return segments.flatMap((s): (Segment | FoldInput)[] => (typeof s !== 'string' && s.tk === 'fold' ? (s.inputs ?? []) : [s]));
}

/** Every distinct reference (`$naam`, `$rij.veld`) in a reading, in reading order. */
export function references(segments: readonly Segment[]): string[] {
  const seen: string[] = [];
  for (const s of withFoldInputs(segments)) {
    if (typeof s !== 'string' && (s.tk === 'name' || s.tk === 'term') && s.ref !== undefined && !seen.includes(s.ref)) seen.push(s.ref);
  }
  return seen;
}
