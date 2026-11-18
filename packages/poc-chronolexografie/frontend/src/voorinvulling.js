/**
 * The application prefilled from the sources. The UI spec of the case's
 * source says per field which source fills it (`state.vormen.aanvraag.prefill`,
 * `<regelwerk>#<article>#<output>`); the cell computes that source with the
 * cells of the case as they know things on the play date (`api.voorinvullen`).
 * A prefilled value is a proposal: the applicant can change it, and what they
 * submit is what the application holds. The field says where the value comes
 * from, or that the applicant changed it.
 *
 * A field the system fills (`state.vormen.aanvraag.systeem`, src/ui_spec.rs) is
 * no proposal: the applicant does not type it. Per field it says `{ waarde }`
 * for a value the system knows itself, `{ bron }` for a source the cell
 * computes as it prefills, and `{}` for the login (inlogstap.js).
 */
import { isUnknown } from '@regelrecht/frontend-shared';
import { toon } from './logboek.js';
import { leesbaar, waarde as leesbareWaarde } from './state.js';
import { alsInvoer, bronRij, verschilTekst, zelfdeTabel } from './tabel.js';

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

/** Whether a source gave the field a value: not empty, and not unknown (RFC-036). */
const gegeven = (v) => v?.waarde !== null && v?.waarde !== undefined && !isUnknown(v.waarde);

/**
 * Per prefilled field the value a source gave, without the empty and the
 * unknown ones: an unknown value is no proposal, so the field stays as the
 * applicant left it. A table holds its decimal cells as numbers (tabel.js
 * `alsInvoer`).
 */
export function voorgevuldeWaarden(voorgevuld) {
  return Object.fromEntries(
    Object.entries(voorgevuld ?? {})
      .filter(([, v]) => gegeven(v))
      .map(([naam, v]) => [naam, alsInvoer(v.waarde)]),
  );
}

/**
 * The line under a prefilled field: which cell the value comes from, or that
 * the applicant changed or emptied it and what the source gives, or why the
 * source gave nothing (`fout`, or an unknown value and what it misses).
 * Empty when no cell gave a value.
 * `celNaam` names a cell by its id. A table (a field with `kolommen`) counts
 * as the source's when it holds the same rows, in any order (tabel.js); a
 * changed one says per row what the source gives.
 */
export function herkomstTekst(voorgevuld, waarde, celNaam, kolommen) {
  if (voorgevuld?.fout) return `Niet vooringevuld: ${voorgevuld.fout}`;
  if (!voorgevuld || voorgevuld.waarde === null || voorgevuld.waarde === undefined) return '';
  const cellen = [...new Set((voorgevuld.herkomst ?? []).map((h) => celNaam(h.door)))];
  const wie = cellen.join(' en ') || 'de bron';
  if (isUnknown(voorgevuld.waarde)) return `Niet vooringevuld: ${wie} geeft ${leesbareWaarde(voorgevuld.waarde)}.`;
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

/**
 * What the system fills (`systeem`), given the login: everything, unless the
 * case logs in through a login step and nobody is logged in (`sessie`). Such
 * an application does not come through the form, as one on paper, and holds
 * what the applicant gives.
 */
export const systeemVult = (inloggen, sessie, systeem) =>
  inloggen?.inlogstap && !sessie ? {} : (systeem ?? {});

/** Per field the system fills from a source, that source: asked as a prefill is. */
export const systeemBronnen = (systeem) =>
  Object.fromEntries(
    Object.entries(systeem ?? {})
      .filter(([, hoe]) => hoe.bron)
      .map(([naam, hoe]) => [naam, hoe.bron]),
  );

/**
 * What the application holds: the form's `waarden`, with what the system
 * fills over them, per field the value it knows, or what the source gave
 * (`voorgevuld`), and empty when the source gave nothing. That goes over
 * what a scenario or the example gives. A field the login fills keeps what
 * the login gave it (`doorInloggen`, the names inlogstap.js fills), and is
 * empty when the login gives it nothing. A field the form hides keeps its
 * value: it is submitted, only not shown.
 */
export function metSysteem(waarden, systeem, voorgevuld, doorInloggen = new Set()) {
  const gegeven = voorgevuldeWaarden(voorgevuld);
  const uit = { ...waarden };
  for (const [naam, hoe] of Object.entries(systeem ?? {})) {
    if ('waarde' in hoe) uit[naam] = hoe.waarde ?? undefined;
    else if (hoe.bron) uit[naam] = gegeven[naam];
    else if (!doorInloggen.has(naam)) uit[naam] = undefined;
  }
  return uit;
}

/**
 * The fields the form shows of `velden`: not those the login fills
 * (`doorInloggen`, inlogstap.js), nor those the UI spec hides (`verborgen`).
 */
export const opHetFormulier = (velden, verborgen, doorInloggen) =>
  (velden ?? []).filter((v) => !doorInloggen.has(v.naam) && !(verborgen ?? []).includes(v.naam));

/** The line under a field the system fills: where its value comes from. */
export function systeemTekst(hoe, voorgevuld, celNaam) {
  if (hoe?.bron) return herkomstTekst(voorgevuld, voorgevuld?.waarde, celNaam);
  return hoe && 'waarde' in hoe ? 'Ingevuld door het systeem.' : '';
}
