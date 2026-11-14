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

/** The choice of no record, as the select names it. */
export const NIET_INGELOGD = 'Niet ingelogd';

/** How the choice names a record: its name with its key, or its key alone. */
export const keuzeLabel = (keuze) =>
  keuze.naam ? `${keuze.naam} (${keuze.waarde})` : String(keuze.waarde);

/**
 * The record of `inloggen` whose key is `waarde`, if any. The key comes as
 * text; the field may hold it as the form wrote it.
 */
export const gekozen = (inloggen, waarde) =>
  waarde === undefined || waarde === null
    ? undefined
    : (inloggen?.keuzes ?? []).find((k) => k.waarde === String(waarde));

/**
 * What the guide says about logging in for an application with `waarden`
 * (the values its form starts from), given the choices the cell offers now
 * (`inloggen`, `state.vormen.aanvraag.inloggen`): which one to pick, in one
 * sentence. Null without a login, or when the values leave its field out.
 */
export function inlogHint(inloggen, waarden) {
  if (!inloggen || !waarden || !(inloggen.veld in waarden)) return null;
  const waarde = waarden[inloggen.veld];
  if (waarde === null || waarde === undefined || waarde === '') {
    return `Laat Ingelogd als op ${NIET_INGELOGD} staan.`;
  }
  const keuze = gekozen(inloggen, waarde);
  return keuze
    ? `Kies bij Ingelogd als ${keuzeLabel(keuze)}.`
    : `Bij Ingelogd als staat niemand met ${waarde}; de cel kent die inlog op deze speeldatum niet.`;
}

/**
 * The line under the choice: which cell recorded the login, on what day,
 * and what else it recorded then, besides the fields of the key and the
 * name (`inloggen.sleutel`, `inloggen.naam`). Empty without a choice.
 */
export function inlogTekst(keuze, inloggen, celNaam) {
  if (!keuze) return '';
  const getoond = new Set([inloggen?.sleutel, inloggen?.naam]);
  const rest = Object.entries(keuze.velden ?? {})
    .filter(([naam]) => !getoond.has(naam))
    .map(([naam, v]) => `${leesbaar(naam)} ${toon(v)}`);
  const wat = rest.length ? `: ${rest.join(', ')}` : '';
  return `Volgens ${celNaam(keuze.cel)} (${keuze.vastgelegd_op})${wat}.`;
}
