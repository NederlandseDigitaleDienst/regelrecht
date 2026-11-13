/**
 * De aanvraag vooringevuld uit de bronnen. Het UI-beleid van de bron van een
 * casus zegt per veld uit welke bron het komt (`state.vormen.aanvraag.vult_voor`,
 * `<regelwerk>#<artikel>#<uitvoer>`); de cel rekent die bron uit met de
 * cellen van de casus zoals ze het op de speeldatum weten (`api.voorinvullen`).
 * Het voorgevulde gegeven is een voorstel: de aanvrager kan het wijzigen, en
 * wat hij indient is wat de aanvraag bevat. Bij het veld staat waar het
 * vandaan komt, of dat hij het wijzigde.
 */
import { leesbaar } from './state.js';

/**
 * What the sources get from the form `waarden`: every field with a value,
 * prefilled ones too. A source may read another prefilled field, as the
 * seats read the designation; a value the applicant changes then reaches it.
 * Asking again with what a source gave changes nothing, so the form settles.
 */
export function bronInvoer(waarden) {
  return Object.fromEntries(Object.entries(waarden ?? {}).filter(([, w]) => w !== undefined));
}

/** Per prefilled field the value a source gave, without the empty ones. */
export function voorgevuldeWaarden(voorgevuld) {
  return Object.fromEntries(
    Object.entries(voorgevuld ?? {})
      .filter(([, v]) => v.waarde !== null && v.waarde !== undefined)
      .map(([naam, v]) => [naam, v.waarde]),
  );
}

/**
 * The line under a prefilled field: which cell the value comes from, or that
 * the applicant changed or emptied it and what the source gives, or why the
 * source gave nothing (`fout`). Empty when no cell gave a value.
 * `celNaam` names a cell by its id.
 */
export function herkomstTekst(voorgevuld, waarde, celNaam) {
  if (voorgevuld?.fout) return `Niet vooringevuld: ${voorgevuld.fout}`;
  if (!voorgevuld || voorgevuld.waarde === null || voorgevuld.waarde === undefined) return '';
  const cellen = [...new Set((voorgevuld.herkomst ?? []).map((h) => celNaam(h.door)))];
  const wie = cellen.join(' en ') || 'de bron';
  if (waarde === voorgevuld.waarde) {
    const antwoorden = [...new Set((voorgevuld.herkomst ?? []).map((h) => leesbaar(h.antwoord)))];
    return `Ingevuld uit ${wie}${antwoorden.length ? ` (${antwoorden.join(', ')})` : ''}.`;
  }
  const wat = waarde === undefined ? 'leeggemaakt' : 'gewijzigd';
  const hoofd = wie.charAt(0).toUpperCase() + wie.slice(1);
  return `Door u ${wat}. ${hoofd} geeft: ${voorgevuld.waarde}.`;
}
