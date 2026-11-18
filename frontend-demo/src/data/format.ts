/**
 * Display formatting for law values, in the language that is on.
 *
 * Two kinds of "nothing" (RFC-036): `null` is an absence the data states
 * ("geen"/"none": no partner, no rent) and the engine's Unknown is a fact
 * nobody supplied ("onbekend"/"unknown"), which names what is missing. They
 * are never shown with the same word, in either language.
 *
 * This module is imported by its own tests and by plain modules that have no
 * component around them, which is why it reads the locale through
 * `currentLocale()` rather than through `useI18n()`.
 */
import { isUnknown, missingFacts } from '@regelrecht/frontend-shared';
import { DEFAULT_LOCALE, currentLocale, localeDef, t } from '../i18n/index.js';
import generatedGlossary from '../i18n/glossary.generated.js';
import type { Produces } from '@regelrecht/frontend-shared/produces';

export { isUnknown, missingFacts };
export type { MissingFact, UnknownValue } from '@regelrecht/frontend-shared';

/**
 * A field's declared type, as far as the demo reads it: `type` and the
 * `type_spec` (`unit`, `precision`). A caller may pass a spec it builds
 * itself (`{ type: 'amount' }`), so `name` is optional here.
 */
export interface FieldSpec {
  name?: string;
  type?: string;
  required?: boolean;
  type_spec?: { unit?: string | null; precision?: unknown } | null;
}

/** A field an article declares (input, output or parameter): a spec with its name. */
export interface DeclaredField extends FieldSpec {
  name: string;
}

/** The `execution` block of an article, only the parts the demo's helpers read. */
export interface ExecutionLike {
  produces?: Produces;
  parameters?: DeclaredField[] | null;
  input?: DeclaredField[] | null;
  output?: DeclaredField[] | null;
}

/**
 * A regelwerk's YAML document, structurally: only the path down to the
 * declared fields. Any fuller law document type is assignable to it.
 */
export interface LawDocLike {
  articles?: Array<{ machine_readable?: { execution?: ExecutionLike | null } | null }> | null;
}

/** The verdict `verdictOf` reads from a law's outputs. */
export type Verdict = boolean | 'unknown' | null;

/**
 * `en-GB` and not `en-US`, deliberately.
 *
 * It gives "22 September 2026" and 24-hour time, which is how a Dutch
 * government screen states a date and a time, and it keeps the reading order
 * of the Dutch original. `en-US` would render "September 22, 2026" and
 * "12:00 AM" and make the demo read as American. The docs writing rules ask
 * for American *spelling* in prose; date order is a separate question and this
 * is the answer to it.
 *
 * The currency stays EUR in both: only the separators and the symbol's
 * position change (`€ 1.654,12` against `€1,654.12`), and `Intl` handles that.
 */
// De tag staat in de talentabel (`src/i18n/index.js`) en niet hier: hij hoort
// bij de taal, net als zijn prefix en zijn naam, en twee lijstjes die allebei
// de talen opsommen lopen vroeg of laat uiteen.

// Built per locale and cached: an `Intl` formatter bakes its locale in at
// construction, so the module-level pair the demo used to have would keep
// formatting in Dutch after a switch.
const formatters = new Map<string, Intl.NumberFormat>();
function intl(kind: 'euro' | 'number'): Intl.NumberFormat {
  const loc = intlLocale();
  const key = `${kind}:${loc}`;
  if (!formatters.has(key)) {
    formatters.set(
      key,
      kind === 'euro'
        ? new Intl.NumberFormat(loc, { style: 'currency', currency: 'EUR' })
        : new Intl.NumberFormat(loc, { maximumFractionDigits: 4 }),
    );
  }
  // Set just above when it was missing.
  return formatters.get(key)!;
}

/** The BCP 47 tag for the active locale, for the few callers that need it. */
export function intlLocale(): string {
  // `localeDef` falls back to the default locale, which is always in the
  // table; its inferred JS type cannot say so.
  return localeDef(currentLocale())!.intl;
}

const euro = { format: (v: number) => intl('euro').format(v) };
const number = { format: (v: number) => intl('number').format(v) };

/** Find the declared field (input/output/parameter) spec for a name in a law doc. */
export function fieldSpec(lawDoc: LawDocLike | null | undefined, name: string): DeclaredField | null {
  for (const article of lawDoc?.articles ?? []) {
    const ex = article.machine_readable?.execution;
    if (!ex) continue;
    for (const list of [ex.output, ex.input, ex.parameters]) {
      const hit = (list ?? []).find((f) => f.name === name);
      if (hit) return hit;
    }
  }
  return null;
}

export function isAmountSpec(spec: FieldSpec | null | undefined): boolean {
  return spec?.type === 'amount' || spec?.type_spec?.unit === 'eurocent';
}

/** Format one value for display, guided by its declared spec when known. */
export function formatValue(value: unknown, spec: FieldSpec | null = null): string {
  if (isUnknown(value) || value === undefined) return t('format.unknown');
  if (value === null) return t('format.none');
  if (typeof value === 'boolean') return value ? t('format.yes') : t('format.no');
  if (typeof value === 'number') {
    if (isAmountSpec(spec)) return euro.format(value / 100);
    if (spec?.type_spec?.unit === 'euro') return euro.format(value);
    if (spec?.type_spec?.unit === 'percentage') return `${number.format(value)}%`;
    if (spec?.type_spec?.unit === 'years' || spec?.type_spec?.unit === 'jaar') return t('format.years', { n: number.format(value) });
    return number.format(value);
  }
  if (typeof value === 'string') {
    if (/^\d{4}-\d{2}-\d{2}$/.test(value)) return formatDate(value);
    return value.replaceAll('_', ' ');
  }
  if (Array.isArray(value)) {
    if (value.length === 0) return t('format.none');
    if (value.every((v) => typeof v !== 'object' || v === null)) return value.map((v) => formatValue(v)).join(', ');
    return t.plural(value.length, 'format.items');
  }
  if (typeof value === 'object') {
    const iso = (value as { iso?: unknown }).iso;
    if (typeof iso === 'string') return formatDate(iso);
    // A record reads best as its values in order (an address: street, number,
    // postcode, city); the keys are in the tooltip/edit sheet.
    const text = Object.values(value)
      .filter((v) => v !== null && v !== undefined && v !== '')
      .map((v) => formatValue(v))
      .join(' ');
    return text.length > 48 ? `${text.slice(0, 45)}…` : text;
  }
  return String(value);
}

/**
 * What an unknown value lacks: "ontbreekt: huurprijs, spaargeld (Wet
 * inkomstenbelasting)". The law is named only when it is not `ownLaw`; an
 * empty string for anything that is not unknown.
 */
export function formatMissing(
  value: unknown,
  { ownLaw = null, lawName = (id: string) => id, group = true }: { ownLaw?: string | null; lawName?: (id: string) => string; group?: boolean } = {},
): string {
  const parts: string[] = [];
  for (const fact of missingFacts(value)) {
    // What another law misses is that law's business: one reference to it, not
    // every fact it lacks. Claudia's precario tile listed eight terrace facts,
    // each followed by the terrace law's name, half of them register values she
    // cannot supply; what she needs to know is that the terrace permit comes first.
    // The data lineage, which is about the detail, passes `group: false` and
    // keeps each fact with its law.
    const label = humanize(fact.name).toLowerCase();
    const part = !fact.law || fact.law === ownLaw ? label : group ? t('format.missing_from_law', { law: lawName(fact.law) }) : `${label} (${lawName(fact.law)})`;
    if (!parts.includes(part)) parts.push(part);
  }
  return parts.length ? t('format.missing', { facts: parts.join(', ') }) : '';
}

/**
 * The verdict of an evaluation's outputs: `true`/`false` when the law decided,
 * `'unknown'` when it could not for lack of facts, `null` when the law has no
 * `voldoet_aan_voorwaarden` output (a tax, a registration: computed for all).
 * An unknown verdict is never read as a yes.
 */
export function verdictOf(outputs: Record<string, unknown> | null | undefined): Verdict {
  if (!outputs || !('voldoet_aan_voorwaarden' in outputs)) return null;
  const v = outputs.voldoet_aan_voorwaarden;
  if (isUnknown(v)) return 'unknown';
  return v === true;
}

export function formatDate(iso: string): string {
  const d = new Date(`${iso}T00:00:00`);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString(intlLocale(), { day: 'numeric', month: 'long', year: 'numeric' });
}

export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(intlLocale(), { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });
}

/** `hoogte_toeslag` -> `Hoogte toeslag`. */
/**
 * Afkortingen die als afkorting geschreven horen te worden. Zonder deze lijst
 * maakt `humanize` van `agp_vergunning_vereist` "Agp vergunning vereist", en
 * dat leest als een woord in plaats van als de accijnsgoederenplaats die het
 * is. Alleen namen die in het corpus voorkomen; een onbekende afkorting krijgt
 * gewoon de gebruikelijke behandeling.
 */
const ABBREVIATIONS = new Map(
  [
    ['agp', 'AGP'],
    ['aow', 'AOW'],
    ['bsn', 'BSN'],
    ['bbz', 'Bbz'],
    ['brp', 'BRP'],
    ['cbs', 'CBS'],
    ['haccp', 'HACCP'],
    ['kvk', 'KvK'],
    ['nvwa', 'NVWA'],
    ['sbi', 'SBI'],
    ['svh', 'SVH'],
    ['vog', 'VOG'],
    ['wia', 'WIA'],
    ['ww', 'WW'],
    ['zvw', 'Zvw'],
  ],
);

function capitalise(words: string[]): string {
  const out = [...words];
  if (!out.length) return '';
  // De eerste letter met een hoofdletter, tenzij het woord al een afkorting is.
  // Non-empty: checked just above.
  const first = out[0]!;
  out[0] = ABBREVIATIONS.has(String(first).toLowerCase()) ? first : first.charAt(0).toUpperCase() + first.slice(1);
  return out.join(' ');
}

/**
 * Namen waarvan het Engels nog niet is vastgesteld, verzameld tijdens het
 * ontwikkelen. `scripts/i18n-report.mjs` en de woordenlijst-fase leunen hierop;
 * in productie blijft de verzameling leeg omdat er dan niets aan toegevoegd
 * wordt wat iemand opvraagt.
 */
export const untranslated = new Set<string>();

export function humanize(
  name: string | null | undefined,
  { lawId = null, locale = currentLocale(), report = true }: { lawId?: string | null; locale?: string; report?: boolean } = {},
): string {
  if (!name) return '';
  const words = String(name).replaceAll('_', ' ').trim().split(/\s+/);
  const dutch = capitalise(words.map((w) => ABBREVIATIONS.get(w.toLowerCase()) ?? w));
  if (locale === DEFAULT_LOCALE) return dutch;

  // Vertaald: eerst de hele naam, dan woord voor woord. Een naam die in geen
  // van beide staat valt terug op het Nederlands, wat een schoonheidsfout is
  // en geen onwaarheid.
  const g = activeGlossary(locale);
  const exact = (lawId === null ? undefined : g.laws?.[lawId]?.[name]) ?? g.names?.[name];
  if (exact) return capitalise(String(exact).split(/\s+/));

  // Alles of niets. Een half vertaald label ("Has partner inkomen") leest als
  // een bug, waar een Nederlands label leest als iets dat nog niet vertaald is.
  const parts = words.map((w) => g.words?.[w] ?? ABBREVIATIONS.get(w.toLowerCase()) ?? null);
  if (parts.every(Boolean)) return capitalise(parts.join(' ').split(/\s+/));

  if (import.meta.env?.DEV && report) untranslated.add(name);
  return dutch;
}

/**
 * A glossary for field names in one language, as `copy-demo-corpus.mjs`
 * generates it: names per law, names for every law, and single words.
 */
export interface Glossary {
  laws?: Record<string, Record<string, string> | undefined>;
  names?: Record<string, string | undefined>;
  words?: Record<string, string | undefined>;
}

/** Wat `humanize` gebruikt als er geen woordenlijst voor de taal is. */
const EMPTY_GLOSSARY: Glossary = { laws: {}, names: {}, words: {} };

/**
 * De woordenlijst voor veldnamen in de taal die aanstaat.
 *
 * Gegenereerd per taal uit `corpus/demo/i18n/glossary.<taal>.yaml` door
 * `copy-demo-corpus.mjs`. Een taal zonder lijst krijgt een lege, en dan valt
 * elk label terug op het Nederlands: een schoonheidsfout, geen onwaarheid.
 *
 * `override` is er voor de tests, die een eigen lijst willen zetten zonder de
 * meegeleverde te raken. Hij geldt voor elke taal, want een test zet er één en
 * kiest daarna de taal; `setGlossary(null)` zet hem terug.
 */
let override: Glossary | null = null;

function activeGlossary(locale: string = currentLocale()): Glossary {
  if (override) return override;
  return (generatedGlossary as Record<string, Glossary | undefined>)[locale] ?? EMPTY_GLOSSARY;
}

export function setGlossary(next: Partial<Glossary> | null | undefined): void {
  override = next ? { ...EMPTY_GLOSSARY, ...next } : null;
  untranslated.clear();
}

/** Eurocent value for a monetary spec; the raw number otherwise. */
export function numericImpact(value: unknown, spec: FieldSpec | null | undefined): number {
  if (typeof value !== 'number') return 0;
  return isAmountSpec(spec) ? value / 100 : value;
}
