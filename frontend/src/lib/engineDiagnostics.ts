/**
 * engineDiagnostics — the engine's problems with a regelwerk, one record
 * each, and the editor's words for them.
 *
 * `loadLawWithDiagnostics` (packages/engine/src/wasm.rs) throws
 * `{ error, diagnostics }`: the sentence the engine has always given, and the
 * same problems apart, each with the article and place it is about. The
 * records are factual and in the engine's English; what the user reads is
 * written here.
 */

/** The place within an article a problem is about. */
export type DiagnosticPlace =
  | { kind: 'article' }
  | { kind: 'output'; name: string }
  | { kind: 'action'; index: number }
  | { kind: 'input'; name: string }
  | { kind: 'definition'; name: string };

/** One problem, as the engine serializes it (packages/engine/src/diagnostic.rs). */
export interface EngineDiagnostic {
  severity: 'error' | 'warning';
  kind: 'structure' | 'load_check' | 'type_check' | 'cross_law';
  law_id: string | null;
  article: string | null;
  place: DiagnosticPlace | null;
  position: { line: number; column: number } | null;
  rule: string | null;
  message: string;
}

/** The records in whatever the engine threw; none when it threw a bare string. */
export function diagnosticsOf(thrown: unknown): EngineDiagnostic[] {
  const list = (thrown as { diagnostics?: unknown } | null)?.diagnostics;
  if (!Array.isArray(list)) return [];
  return list.filter(
    (d): d is EngineDiagnostic => !!d && typeof d === 'object' && typeof (d as EngineDiagnostic).message === 'string',
  );
}

/** The place in the editor's words: "Uitkomst hoogte", "Actie 3". */
export function placeLabel(place: DiagnosticPlace | null): string | null {
  switch (place?.kind) {
    case 'output':
      return `Uitkomst ${place.name}`;
    case 'input':
      return `Invoer ${place.name}`;
    case 'definition':
      return `Definitie ${place.name}`;
    case 'action':
      return `Actie ${place.index}`;
    default:
      return null;
  }
}

/**
 * What each rule of the type check is about, for an author (RFC-036 for the
 * N rules, RFC-037 for the T rules; the rules themselves are in
 * packages/engine/src/typecheck.rs). The engine's message says what exactly
 * is wrong; this says what kind of mistake it is.
 */
const RULE_TEXT: Record<string, string> = {
  N1: 'Een veld dat nooit leeg is, wordt met leeg (null) vergeleken.',
  N2: 'Er staat een lege waarde (null) op een plek waar die niet mag.',
  N3: 'Een IF zonder default kan leeg uitkomen, en de uitkomst mag niet leeg zijn.',
  N4: 'Een waarde die leeg kan zijn, wordt gebruikt zonder eerst te toetsen of ze er is.',
  N5: 'Een waarde die leeg kan zijn, komt binnen op een plek die niet leeg mag zijn.',
  T1: 'Een rekenkundige bewerking of vergelijking krijgt iets dat geen getal is.',
  T2: 'Een logische bewerking of IF-voorwaarde krijgt iets dat geen ja/nee-waarde is.',
  T3: 'Er worden waarden van verschillend type met elkaar vergeleken.',
  T4: 'De waarde heeft een ander type dan de uitkomst.',
  T5: 'De aanroep van een artikel past niet bij wat dat artikel oplevert.',
};

/** The kind of mistake, in Dutch; null for a problem the editor has no words for yet. */
export function ruleText(diagnostic: EngineDiagnostic): string | null {
  if (diagnostic.rule && RULE_TEXT[diagnostic.rule]) return RULE_TEXT[diagnostic.rule] ?? null;
  if (diagnostic.kind === 'structure') return 'De tekst kan niet als regelwerk worden gelezen.';
  return null;
}

/** The distinct articles the records name, in the order they come. */
export function articlesOf(diagnostics: readonly EngineDiagnostic[]): string[] {
  return [...new Set(diagnostics.flatMap((d) => (d.article ? [d.article] : [])))];
}
