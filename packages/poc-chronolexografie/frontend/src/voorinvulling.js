/**
 * The application prefilled from the sources. The UI spec of the case's
 * source says per field which source fills it (`state.vormen.aanvraag.prefill`,
 * `<regelwerk>#<article>#<output>`); the cell computes that source with the
 * cells of the case as they know things on the play date (`api.voorinvullen`).
 * A prefilled value is a proposal: the applicant can change it, and what they
 * submit is what the application holds. The field says where the value comes
 * from, or that the applicant changed it.
 */
import { toon } from './logboek.js';
import { leesbaar } from './state.js';
import { bronRij, verschilTekst, zelfdeTabel } from './tabel.js';

/**
 * What the application form starts from. `scenario` is what a scenario gives
 * the form (the case's example with the step's own values over it), `invoer`
 * the step's own values alone, and `voorgevuld` what the sources gave. A
 * prefilled field takes the source over the example, since the example is no
 * choice of the applicant; a value the step sets is, and wins.
 */
export function beginWaarden(scenario, invoer, voorgevuld) {
  const eigen = Object.fromEntries(
    Object.keys(invoer ?? {})
      .filter((naam) => naam in (scenario ?? {}))
      .map((naam) => [naam, scenario[naam]]),
  );
  return { ...(scenario ?? {}), ...voorgevuldeWaarden(voorgevuld), ...eigen };
}

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
 * `celNaam` names a cell by its id. A table (a field with `kolommen`) counts
 * as the source's when it holds the same rows, in any order (tabel.js); a
 * changed one says per row what the source gives.
 */
export function herkomstTekst(voorgevuld, waarde, celNaam, kolommen) {
  if (voorgevuld?.fout) return `Niet vooringevuld: ${voorgevuld.fout}`;
  if (!voorgevuld || voorgevuld.waarde === null || voorgevuld.waarde === undefined) return '';
  const cellen = [...new Set((voorgevuld.herkomst ?? []).map((h) => celNaam(h.door)))];
  const wie = cellen.join(' en ') || 'de bron';
  const tabel = Array.isArray(kolommen) && Array.isArray(voorgevuld.waarde);
  if (tabel ? zelfdeTabel(voorgevuld.waarde, waarde, kolommen) : waarde === voorgevuld.waarde) {
    const antwoorden = [...new Set((voorgevuld.herkomst ?? []).map((h) => leesbaar(h.antwoord)))];
    return `Ingevuld uit ${wie}${antwoorden.length ? ` (${antwoorden.join(', ')})` : ''}.`;
  }
  const hoofd = wie.charAt(0).toUpperCase() + wie.slice(1);
  if (waarde === undefined) {
    const bron = tabel ? voorgevuld.waarde.map((rij) => bronRij(rij, kolommen)).join('; ') : toon(voorgevuld.waarde);
    return `Door u leeggemaakt. ${hoofd} geeft: ${bron}.`;
  }
  if (tabel) return `Door u gewijzigd. ${hoofd} geeft ${verschilTekst(voorgevuld.waarde, waarde, kolommen)}.`;
  return `Door u gewijzigd. ${hoofd} geeft: ${toon(voorgevuld.waarde)}.`;
}
