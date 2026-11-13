/**
 * Logging in to apply, without a real means of login. The case names a fact
 * a cell keeps, such as the authentication an accredited service recorded
 * (`state.vormen.aanvraag.inloggen`); the applicant picks one of its records,
 * and its key goes into a field of the application. What the cell recorded
 * at that login shows under the choice. The prefill then reads that field
 * (voorinvulling.js).
 */
import { toon } from './logboek.js';
import { leesbaar } from './state.js';

/** How the choice names a record: its name with its key, or its key alone. */
export const keuzeLabel = (keuze) =>
  keuze.naam ? `${keuze.naam} (${keuze.waarde})` : String(keuze.waarde);

/** The record of `inloggen` whose key is `waarde`, if any. */
export const gekozen = (inloggen, waarde) =>
  (inloggen?.keuzes ?? []).find((k) => k.waarde === waarde);

/**
 * The line under the choice: which cell recorded the login, on what day,
 * and what else it recorded then. Empty without a choice.
 */
export function inlogTekst(keuze, celNaam) {
  if (!keuze) return '';
  const bekend = new Set([keuze.waarde, keuze.naam]);
  const rest = Object.entries(keuze.velden ?? {})
    .filter(([, v]) => !bekend.has(v))
    .map(([naam, v]) => `${leesbaar(naam)} ${toon(v)}`);
  const wat = rest.length ? `: ${rest.join(', ')}` : '';
  return `Volgens ${celNaam(keuze.cel)} (${keuze.vastgelegd_op})${wat}.`;
}
