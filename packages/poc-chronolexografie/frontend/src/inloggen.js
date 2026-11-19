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
export const NOT_LOGGED_IN = 'Niet ingelogd';

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
 * `waarden` of the application with the login field taken from `keuze`, the
 * key chosen under Ingelogd als before (sessie.js), so that the choice holds
 * for the page and not only for one form. Only where nothing else says who
 * applies: unchanged for a case with a login step, without a choice, when
 * the form already holds the field, or when `zelf` says the field is not
 * the choice's to fill (the applicant changed it in this form, or the
 * scenario is at an application step and names its own values).
 */
export function metKeuze(inloggen, keuze, waarden, zelf) {
  if (!inloggen || inloggen.inlogstap || keuze === undefined || zelf) return waarden;
  if (waarden[inloggen.veld] !== undefined) return waarden;
  return { ...waarden, [inloggen.veld]: keuze };
}

/**
 * What the guide says about logging in for an application with `waarden`
 * (the values its form starts from), given the choices the cell offers now
 * (`inloggen`, `state.vormen.aanvraag.inloggen`): which one the form holds,
 * in one sentence; the form already chose it from the same values. Null without a login, or when the values leave its field out.
 */
export function loginHint(inloggen, waarden) {
  // A login step has steps of its own in the guide (scenarioGuides.js).
  if (!inloggen || inloggen.inlogstap || !waarden || !(inloggen.veld in waarden)) return null;
  const waarde = waarden[inloggen.veld];
  if (waarde === null || waarde === undefined || waarde === '') {
    return `Laat Ingelogd als op ${NOT_LOGGED_IN} staan.`;
  }
  const keuze = gekozen(inloggen, waarde);
  return keuze
    ? `Laat Ingelogd als op ${keuzeLabel(keuze)} staan.`
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
