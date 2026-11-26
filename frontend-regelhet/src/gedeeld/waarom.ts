// Why a decision came out so, in words: who filled each open term of it,
// as the engine reports it (`FilledBy` in packages/engine/src/engine.rs).
import { jaNee } from '@regelrecht/frontend-shared/chronolex/waarden';
import { artikelNaam } from './keten';

/** A regulation or policy article that filled a term. */
interface Artikel {
  law_id: string;
  article: string;
}

const isObject = (v: unknown): v is Record<string, unknown> => v !== null && typeof v === 'object';
const isArtikel = (v: unknown): v is Artikel =>
  isObject(v) && typeof v.law_id === 'string' && typeof v.article === 'string';

/**
 * Who filled an open term, as a line: a finding of the one who decided
 * (`beslisser`, the name of the cell that took the decision, such as a
 * municipality or a court), a regulation, or a policy that took the answer of
 * a cell. `celNaam` names a cell by its id.
 */
export function ingevuldDoor(filledBy: unknown, celNaam: (cel: string) => string, beslisser: string): string {
  if (filledBy === 'finding') return `bevinding van ${beslisser}`;
  if (!isObject(filledBy)) return 'niet ingevuld';
  if (isArtikel(filledBy.regulation)) {
    return `ingevuld door ${artikelNaam(`${filledBy.regulation.law_id}#${filledBy.regulation.article}`)}`;
  }
  const antwoord = filledBy.answer;
  if (isArtikel(antwoord) && isObject(antwoord)) {
    const answers = Array.isArray(antwoord.answers) ? antwoord.answers : [];
    const cellen = [
      ...new Set(
        answers.flatMap((a: unknown) => (isObject(a) && typeof a.answerer === 'string' ? [celNaam(a.answerer)] : [])),
      ),
    ];
    const met = cellen.length ? `, met het antwoord van ${cellen.join(' en ')}` : ', zonder antwoord van een cel';
    return `ingevuld door ${artikelNaam(`${antwoord.law_id}#${antwoord.article}`)}${met}`;
  }
  return 'niet ingevuld';
}

/** An answer of a cell, as a value: yes, no, or that the cell was silent and `beslisser` judges. */
export const antwoordTekst = (v: unknown, beslisser: string): string =>
  v === null || v === undefined ? `geen antwoord (${beslisser} oordeelt)` : jaNee(v);
