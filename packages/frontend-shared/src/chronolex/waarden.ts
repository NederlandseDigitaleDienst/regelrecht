/**
 * Values of the chronolexografie kern in words, for the apps that run its
 * steps: the poc (packages/poc-chronolexografie/frontend) and the
 * RegelHet (frontend-regelhet). A name from the law, a yes or no,
 * a decimal as a chronolexogram holds it, an unknown value (RFC-036), and a
 * decision held as a value.
 */
import { isUnknown, missingFacts } from '../values';

/** A name from the law readable: `laatste_verlofdag` → `laatste verlofdag`. */
export const leesbaar = (naam: unknown): string => String(naam).replaceAll('_', ' ');

/** `verlof verleend` → `Verlof verleend`. */
export const hoofdletter = (tekst: string): string => tekst.charAt(0).toUpperCase() + tekst.slice(1);

/** `Verlof verleend` → `verlof verleend`, to go on within a sentence; an abbreviation such as `BSN` keeps its capitals. */
export const kleineLetter = (tekst: string): string => (/^[A-Z][a-z]/.test(tekst) ? tekst.charAt(0).toLowerCase() + tekst.slice(1) : tekst);

/** Regelwerken that go by their abbreviation everywhere. */
const AFKORTINGEN: Record<string, string> = {
  algemene_wet_bestuursrecht: 'Awb',
  besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw: 'BABW',
};

/**
 * The name of a regelwerk: its abbreviation, the name it carries itself
 * (`naamVan`, from the case), or its `$id` readable with a capital. A name
 * that refers to a value of the regelwerk (`'#wet_naam'`) is no name.
 */
export function wetNaam(wet: string, naamVan?: (wet: string) => string | null | undefined): string {
  const afkorting = AFKORTINGEN[wet];
  if (afkorting) return afkorting;
  const naam = naamVan?.(wet);
  if (naam && !naam.startsWith('#')) return naam;
  return hoofdletter(leesbaar(wet));
}

/** `leerplichtwet_1969`, `13a` → `Leerplichtwet 1969 artikel 13a`. */
export const artikelNaam = (wet: string, artikel: string, naamVan?: (wet: string) => string | null | undefined): string =>
  `${wetNaam(wet, naamVan)} artikel ${artikel}`;

/** An outcome readable: `true` → `ja`, `false` → `nee`. */
export const jaNee = (v: unknown): string => (v === true ? 'ja' : v === false ? 'nee' : String(v));

/** A decimal as a chronolexogram holds it: `{"__decimal": "…"}`. */
export interface Decimaal {
  __decimal: string;
}

/** Whether `v` is a decimal as a chronolexogram holds it. */
export const isDecimaal = (v: unknown): v is Decimaal =>
  v !== null && typeof v === 'object' && Object.keys(v).length === 1 && '__decimal' in v;

/** A decision held as a value (`{basis, fields, findings, outputs}`). */
export interface Besluitwaarde {
  basis: string;
  fields: Record<string, unknown>;
  findings: Record<string, unknown>;
  outputs: Record<string, unknown>;
}

/**
 * Whether `v` is a decision held as a value, such as the judgment holds when
 * the court provides in the case itself (Awb 8:72, third paragraph, under b).
 */
export const isBesluitwaarde = (v: unknown): v is Besluitwaarde => {
  if (v === null || typeof v !== 'object') return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o.basis === 'string' &&
    ['fields', 'findings', 'outputs'].every((k) => o[k] !== null && typeof o[k] === 'object')
  );
};

/**
 * A value readable: a decimal as its number, a decision held as a value as
 * its basis with its outputs (a decision in those the same, in brackets), an
 * unknown value (RFC-036) as unknown with what it misses, the rest as JSON.
 */
export function waarde(v: unknown): string {
  if (isDecimaal(v)) return v.__decimal;
  if (isUnknown(v)) {
    // As the editor (outputFormat.ts) and the demo (format.ts) write it, each
    // name once: the engine may report one fact from two laws.
    const ontbreekt = [...new Set(missingFacts(v).map((f) => leesbaar(f.name)))];
    return ontbreekt.length ? `onbekend (ontbreekt: ${ontbreekt.join(', ')})` : 'onbekend';
  }
  if (isBesluitwaarde(v)) {
    const uitvoer = Object.entries(v.outputs).map(([naam, w]) => {
      const tekst = waarde(w);
      return `${leesbaar(naam)}: ${isBesluitwaarde(w) ? `(${tekst})` : tekst}`;
    });
    return [`besluit onder ${v.basis}`, ...uitvoer].join('; ');
  }
  return JSON.stringify(v);
}

/** A value from the chain in words: yes, no, a date or a number. */
export function toon(v: unknown): string {
  if (v === true) return 'ja';
  if (v === false) return 'nee';
  if (v == null) return 'geen waarde';
  if (Array.isArray(v)) return v.length ? v.map((x) => toon(x)).join(', ') : 'geen';
  if (typeof v === 'object' && 'basis' in v) return `een besluit onder ${String((v as { basis: unknown }).basis)}`;
  if (typeof v === 'string') return v;
  return waarde(v);
}

/** Fields without the empty ones (`undefined`): a submission leaves those out. */
export const zonderLeeg = (velden: Record<string, unknown>): Record<string, unknown> =>
  Object.fromEntries(Object.entries(velden).filter(([, v]) => v !== undefined));
