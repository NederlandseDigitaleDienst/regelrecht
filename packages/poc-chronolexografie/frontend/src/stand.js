/**
 * What the case's cell knows about one of its decisions, as the UI spec of
 * the case puts it beside the decision (`state.vormen.per_besluit`,
 * src/ui_spec.rs): per view the answers of the cell for its sources, with
 * the id of the decision as the parameter each answer names (`besluit`),
 * and what the authority gives besides in the fields the view asks. The
 * cell is asked on the play date; nothing is recorded.
 */
import { api } from './api.js';
import { hoofdletter, jaNee, leesbaar, longDate, waarde } from './state.js';

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

/**
 * The parameters of `antwoord` for the decision `besluit`: its id under the
 * parameter the view names, and of the values `gegeven` those the article of
 * the answer takes (`parameters`). An empty value stays out.
 */
export function invoerVoor(antwoord, besluit, gegeven = {}) {
  const neemt = new Set(antwoord.parameters ?? []);
  const extra = Object.entries(gegeven).filter(([naam, w]) => neemt.has(naam) && w !== undefined && w !== '');
  return { ...Object.fromEntries(extra), [antwoord.besluit]: besluit };
}

const euro = (eurocent) =>
  new Intl.NumberFormat('nl-NL', { style: 'currency', currency: 'EUR' }).format(eurocent / 100);

/**
 * An answer as the page shows it, by the type and unit of its output: an
 * amount in eurocent or euro as euros, a date written out, yes or no, and
 * `geen` for an empty value. Anything else as `waarde` writes it.
 */
export function toonWaarde(v, antwoord = {}) {
  if (v === null || v === undefined) return 'geen';
  if (typeof v === 'boolean') return hoofdletter(jaNee(v));
  if (typeof v === 'number' && antwoord.type === 'amount') {
    if (antwoord.eenheid === 'eurocent') return euro(v);
    if (antwoord.eenheid === 'euro') return euro(v * 100);
  }
  if (typeof v === 'string' && ISO_DATE.test(v)) return longDate(v);
  if (typeof v === 'string') return v;
  return waarde(v);
}

/** The label of an answer: the name of its field, readable. */
export const label = (antwoord) => hoofdletter(leesbaar(antwoord.veld));

/**
 * Per answer of `weergave` what the cell answers for the decision `besluit`
 * on `datum`, with the values `gegeven`: `{ ...antwoord, kreeg }`, or with
 * `fout` where the law holds no such source or the cell refuses. `vraag`
 * asks the cell, as `api.antwoord`.
 */
export function vraagStand(weergave, besluit, datum, gegeven = {}, vraag = api.antwoord) {
  return Promise.all(
    (weergave.antwoorden ?? []).map(async (a) => {
      if (a.fout) return { ...a, kreeg: null };
      try {
        const g = await vraag(null, a.bron, invoerVoor(a, besluit, gegeven), datum);
        return { ...a, kreeg: g.waarde };
      } catch (e) {
        return { ...a, kreeg: null, fout: e?.message ?? String(e) };
      }
    }),
  );
}

/**
 * The answer of a view for `bron` (`<regelwerk>#<article>#<output>`), for its
 * type and unit, or an empty object when no view shows that source.
 */
export const soortVan = (bron, weergaven = []) =>
  weergaven.flatMap((w) => w.antwoorden ?? []).find((a) => a.bron === bron && !a.fout) ?? {};
