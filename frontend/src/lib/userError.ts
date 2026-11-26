/**
 * userError — split a failure into what a user reads and what a developer
 * needs.
 *
 * A thrown error's own text is written for whoever debugs it: an engine
 * type-check finding, a response body, a stack of nested causes. Shown as-is
 * it fills the screen and tells an author nothing they can act on. A
 * `UserError` keeps that text as `details`, for the technical-details sheet
 * (`ErrorDetailsSheet`), and puts a short Dutch sentence beside it as
 * `summary`.
 *
 * The describers below never drop the original text: whatever is not
 * recognised still ends up in `details`.
 */
import { ApiError } from './apiFetch';

export interface UserError {
  /** One or two Dutch sentences for the message itself. */
  summary: string;
  /** The technical text, or null when the summary already says everything. */
  details: string | null;
}

/** The text of whatever was thrown: an Error, or the bare string a WASM throw produces. */
export function rawErrorText(e: unknown): string {
  if (typeof e === 'string') return e;
  const message = (e as { message?: unknown } | null)?.message;
  return typeof message === 'string' && message ? message : String(e);
}

/** Engine messages (packages/engine/src/error.rs) an author can act on. */
const ENGINE_PATTERNS: Array<[RegExp, (m: RegExpMatchArray) => string]> = [
  [
    /fails the type check \((\d+) findings?\)/,
    (m) => (m[1] === '1'
      ? 'De engine vond een fout in het regelwerk.'
      : `De engine vond ${m[1]} fouten in het regelwerk.`),
  ],
  [/YAML parse error/, () => 'Het regelwerk is niet geldig en kan niet worden gelezen.'],
  [/YAML exceeds maximum size/, () => 'Het regelwerk is te groot om uit te voeren.'],
  [
    /No version of law '([^']+)' in force on (\S+)/,
    (m) => `Er is geen versie van ${m[1]} die geldt op ${m[2]}.`,
  ],
  [/Law '([^']+)' not found/, (m) => `Het regelwerk ${m[1]} is niet geladen.`],
  [/Law not found: (\S+)/, (m) => `Het regelwerk ${m[1]} is niet geladen.`],
  [/Circular reference detected/, () => 'De regels verwijzen in een kring naar elkaar.'],
  [/Division by zero/, () => 'Een berekening deelt door nul.'],
];

function engineSummary(raw: string, fallback: string): string {
  let summary = fallback;
  for (const [pattern, describe] of ENGINE_PATTERNS) {
    const match = raw.match(pattern);
    if (match) {
      summary = describe(match);
      break;
    }
  }
  const articles = articlesNamed(raw);
  if (articles.length === 0) return summary;
  return `${summary} Kijk bij ${articles.length === 1 ? 'artikel' : 'de artikelen'} ${listInDutch(articles)}.`;
}

/**
 * The articles the engine's text points at, in the two shapes it uses. Other
 * messages mention an article too, of another regelwerk or one that does not
 * exist, so only these shapes count. An article number can hold a space
 * ("B 1").
 */
function articlesNamed(raw: string): string[] {
  // The type check lists its findings one per line:
  // "  <law> article <number> output 'x': [N1] ...".
  const perFinding = [...raw.matchAll(/^\s+\S+ article (.+?) (?:output|action|input|definition)\b/gm)].flatMap((m) => (m[1] ? [m[1]] : []));
  if (perFinding.length > 0) return [...new Set(perFinding)];
  // A load check starts with the article: "article 2: ...",
  // "Failed to load law: article 2, output x: ...".
  const leading = raw.match(/(?:^|: )article ([^:,\n]+)(?=[:,])/);
  return leading?.[1] ? [leading[1]] : [];
}

function listInDutch(items: string[]): string {
  if (items.length < 2) return items.join('');
  return `${items.slice(0, -1).join(', ')} en ${items[items.length - 1]}`;
}

/** The engine refused the regelwerk (parse or type-check error). */
export function describeEngineLoadError(e: unknown): UserError {
  const raw = rawErrorText(e);
  return {
    summary: engineSummary(raw, 'Het regelwerk bevat een fout die eerst moet worden opgelost.'),
    details: raw,
  };
}

/** What a run says about a scenario that names no output; it needs no details. */
export const SCENARIO_WITHOUT_OUTPUT = 'Dit scenario noemt geen uitkomst om te berekenen.';

/** How a run reports that the regelwerk it executes was refused (scenario/run.ts). */
const NOT_LOADED = /^Regelwerk '([^']+)' is niet geladen: /;

/** A scenario run failed, in the engine or before it got there. */
export function describeExecutionError(e: unknown): UserError {
  const raw = rawErrorText(e);
  if (raw === SCENARIO_WITHOUT_OUTPUT) return { summary: raw, details: null };
  const notLoaded = raw.match(NOT_LOADED);
  if (notLoaded) {
    return {
      summary: `Het regelwerk ${notLoaded[1]} bevat een fout en is daarom niet geladen.`,
      details: raw,
    };
  }
  return {
    summary: engineSummary(raw, 'Het scenario kon niet worden uitgevoerd.'),
    details: raw,
  };
}

function saveSummaryForStatus(status: number): string {
  if (status === 403) return 'Je hebt geen rechten om dit regelwerk op te slaan.';
  if (status === 400 || status === 422) return 'De server heeft het regelwerk geweigerd.';
  if (status >= 500) return 'De server kon het regelwerk niet opslaan. Probeer het later opnieuw.';
  return 'Het regelwerk kon niet worden opgeslagen.';
}

/**
 * Saving the regelwerk failed. A plain `Error` is one the editor wrote itself
 * (the 412 conflict text), so its message is the summary.
 */
export function describeSaveError(e: unknown): UserError {
  if (e instanceof ApiError) {
    const body = e.body || e.message;
    return {
      summary: saveSummaryForStatus(e.status),
      details: `HTTP ${e.status}\n${body}`,
    };
  }
  // `fetch` rejects with a TypeError when the request never got a response;
  // these are the messages of Chromium, Firefox and Safari. Any other
  // TypeError is a bug, and says nothing about the connection.
  if (e instanceof TypeError && /Failed to fetch|NetworkError|Load failed/.test(e.message)) {
    return {
      summary: 'Geen verbinding met de server. Controleer je verbinding en probeer het opnieuw.',
      details: rawErrorText(e),
    };
  }
  if (e instanceof TypeError) {
    return { summary: saveSummaryForStatus(0), details: rawErrorText(e) };
  }
  return { summary: rawErrorText(e), details: null };
}
