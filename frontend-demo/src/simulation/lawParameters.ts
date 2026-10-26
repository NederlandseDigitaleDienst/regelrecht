/**
 * The tunable constants of a law: the numeric `definitions` of its articles
 * (a threshold, a rate, an amount). The simulation lets the presenter change
 * them for one run ("what if the drempelinkomen goes up?") without touching
 * the corpus: the law document is cloned with the new values and reloaded in
 * the engine for the duration of the run.
 */
import * as yaml from 'js-yaml';

/** The fields of a parsed law document this module reads (law-format keys). */
export interface DefinitionsDoc {
  articles?: Array<{
    number?: unknown;
    machine_readable?: { definitions?: Record<string, unknown> | null } | null;
  }> | null;
}

/** One tunable constant of a law. */
export interface OverridableDefinition {
  key: string;
  value: number;
  /** The article number that defines it, as a string ('' when it has none). */
  article: string;
}

/** Law constants changed for a run: definition key -> value. */
export type DefinitionOverrides = Record<string, number>;

/** How a definition is displayed; see `definitionKind`. */
export type DefinitionKind = 'percentage' | 'eurocent' | 'number';

/**
 * @param doc parsed law YAML
 */
export function overridableDefinitions(doc: DefinitionsDoc | null | undefined): OverridableDefinition[] {
  const out: OverridableDefinition[] = [];
  const seen = new Set<string>();
  for (const article of doc?.articles ?? []) {
    const defs = article.machine_readable?.definitions ?? {};
    for (const [key, value] of Object.entries(defs)) {
      if (typeof value !== 'number' || !Number.isFinite(value) || seen.has(key)) continue;
      seen.add(key);
      out.push({ key, value, article: String(article.number ?? '') });
    }
  }
  return out;
}

/**
 * A copy of the law document with `definitions` replaced by the overrides,
 * serialised as YAML for `engine.loadLaw`. Keys the law does not define are
 * ignored; a non-numeric override is ignored too.
 */
export function applyOverrides(doc: DefinitionsDoc, overrides: Record<string, unknown> | null | undefined): string {
  const copy = JSON.parse(JSON.stringify(doc)) as DefinitionsDoc;
  for (const article of copy.articles ?? []) {
    const defs = article.machine_readable?.definitions;
    if (!defs) continue;
    for (const [key, value] of Object.entries(overrides ?? {})) {
      if (key in defs && typeof defs[key] === 'number' && typeof value === 'number' && Number.isFinite(value)) defs[key] = value;
    }
  }
  return yaml.dump(copy, { lineWidth: -1, noRefs: true });
}

/** Only the overrides that differ from the law's own values. */
export function effectiveOverrides(doc: DefinitionsDoc | null | undefined, overrides: Record<string, unknown> | null | undefined): DefinitionOverrides {
  const own: Record<string, number> = Object.fromEntries(overridableDefinitions(doc).map((d) => [d.key, d.value]));
  const out: DefinitionOverrides = {};
  for (const [key, value] of Object.entries(overrides ?? {})) {
    if (key in own && typeof value === 'number' && Number.isFinite(value) && value !== own[key]) out[key] = value;
  }
  return out;
}

/** Guess how a definition is displayed from its name and magnitude. */
export function definitionKind(key: string, value: number): DefinitionKind {
  const k = key.toLowerCase();
  if (/percentage|percent|factor|tarief_perc|_pct/.test(k)) return 'percentage';
  if (/bedrag|drempel|inkomen|premie|grens|max|min|norm|kop|toeslag|vermogen|huur|kosten|tarief|vrijstelling|franchise|uitkering/.test(k) && Number.isInteger(value) && Math.abs(value) >= 100) {
    return 'eurocent';
  }
  return 'number';
}
