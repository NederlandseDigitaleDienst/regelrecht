/**
 * A field that holds a table (schema v0.8.20): an array of rows, each an
 * object with a value per column. The law declares the columns (`columns`,
 * here `kolommen`: `{ naam, type }`), each of a scalar type, and every column
 * may hold null; a row that lacks a column holds null there.
 *
 * Two tables are the same when they hold the same rows, whatever their order:
 * the law reads a table as a set of rows (a FOREACH over it, a filter on a
 * column), so moving a row changes nothing it decides. Rows are matched on
 * their content first; what is left is paired on the key columns, so a
 * change in one row reads as a change in that row and not as one row gone and
 * another added.
 *
 * The schema has no marking for a key, so the key columns follow from the
 * types: a column that counts something (`number`, `amount`) is what a row
 * says, and every other column (text, a date, yes or no) is what it is about.
 * That holds without naming any column of any law.
 */
import { toon } from './logboek.js';
import { isDecimaal, leesbaar } from './state.js';

/** The types of a column that counts something: no part of a key. */
const MAAT = ['number', 'amount'];

/** The names of the columns that say which row a row is. */
export const sleutelKolommen = (kolommen) => kolommen.filter((k) => !MAAT.includes(k.type)).map((k) => k.naam);

/** A cell as it is compared: absent is null, a decimal is its number. */
const celWaarde = (v) => {
  if (v === undefined) return null;
  return isDecimaal(v) ? Number(v.__decimal) : v;
};

/** The values of `rij` in the columns `namen`, as one text to compare. */
const inhoud = (rij, namen) => JSON.stringify(namen.map((n) => celWaarde(rij?.[n])));

/**
 * What differs between the table the source gives (`bron`) and the one the
 * applicant holds (`eigen`): rows paired on their key that differ in another
 * column (`gewijzigd`, `{ bron, eigen }`), rows of the source without a pair
 * (`ontbreekt`), and rows the applicant added (`extra`).
 */
export function tabelVerschil(bron, eigen, kolommen) {
  const alle = kolommen.map((k) => k.naam);
  const sleutels = sleutelKolommen(kolommen);
  const rest = [...(eigen ?? [])];
  const zonderGelijke = (bron ?? []).filter((rij) => {
    const i = rest.findIndex((e) => inhoud(e, alle) === inhoud(rij, alle));
    if (i < 0) return true;
    rest.splice(i, 1);
    return false;
  });
  const gewijzigd = [];
  const ontbreekt = [];
  for (const rij of zonderGelijke) {
    const i = sleutels.length ? rest.findIndex((e) => inhoud(e, sleutels) === inhoud(rij, sleutels)) : -1;
    if (i < 0) ontbreekt.push(rij);
    else gewijzigd.push({ bron: rij, eigen: rest.splice(i, 1)[0] });
  }
  return { gewijzigd, ontbreekt, extra: rest };
}

/** Whether two tables hold the same rows, in whatever order. */
export function zelfdeTabel(a, b, kolommen) {
  if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) return false;
  const { gewijzigd, ontbreekt, extra } = tabelVerschil(a, b, kolommen);
  return gewijzigd.length + ontbreekt.length + extra.length === 0;
}

/** A cell as the table shows it: as the app shows values, an empty one as nothing. */
export const celTekst = (v) => (v === null || v === undefined ? '' : toon(v));

/** The filled cells of `rij` in the columns `namen`, in one line. */
const rijTekst = (rij, namen) =>
  namen
    .map((n) => celTekst(rij?.[n]))
    .filter(Boolean)
    .join(', ');

/** A row of the source in one line: every filled cell. */
export const bronRij = (rij, kolommen) => rijTekst(rij, kolommen.map((k) => k.naam));

/**
 * What the source gives where the applicant's table differs, as the rest of
 * a sentence after "… geeft": per changed row the columns that differ, then
 * the rows the source has that the table lacks, then those it does not have.
 */
export function verschilTekst(bron, eigen, kolommen) {
  const { gewijzigd, ontbreekt, extra } = tabelVerschil(bron, eigen, kolommen);
  const sleutels = sleutelKolommen(kolommen);
  const delen = [
    ...gewijzigd.map(({ bron: b, eigen: e }) => {
      const anders = kolommen
        .filter((k) => JSON.stringify(celWaarde(b[k.naam])) !== JSON.stringify(celWaarde(e[k.naam])))
        .map((k) => `${leesbaar(k.naam)} ${toon(celWaarde(b[k.naam]))}`);
      return `bij ${rijTekst(b, sleutels)}: ${anders.join(', ')}`;
    }),
    ...ontbreekt.map((rij) => `ook ${bronRij(rij, kolommen)}`),
    ...extra.map((rij) => `niet ${rijTekst(rij, sleutels.length ? sleutels : kolommen.map((k) => k.naam))}`),
  ];
  return delen.join('; ');
}

/** A column as a field of the form: of its type, and it may stay empty. */
export const kolomVeld = (kolom) => ({ naam: kolom.naam, type: kolom.type, mag_leeg: true, basis: '' });
