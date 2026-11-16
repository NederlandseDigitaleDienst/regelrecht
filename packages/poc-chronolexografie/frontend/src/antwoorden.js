/**
 * The answers a scenario expects of a cell after a step (`antwoorden` in the
 * case YAML), such as what a payment leaves outstanding: an answer of a
 * reduction, which no decision holds. The guide asks the cell for each one
 * on the play date of the step, as tests/scenarios/ does, and shows what it
 * answered beside what the scenario expects (ScenarioGuide.vue). Nothing is
 * recorded.
 */
import { api } from './api.js';
import { isDecimaal } from './state.js';

// A value as the comparison sees it: a decimal by its value, whatever its
// scale, and still apart from a plain number; a record whatever the order of
// its keys.
const vergelijkbaar = (v) => {
  if (isDecimaal(v)) return { __decimal: Number(v.__decimal) };
  if (Array.isArray(v)) return v.map(vergelijkbaar);
  if (v !== null && typeof v === 'object') {
    return Object.fromEntries(
      Object.keys(v)
        .sort()
        .map((k) => [k, vergelijkbaar(v[k])]),
    );
  }
  return v;
};

/**
 * Whether the cell's answer `kreeg` is the value the scenario expects, as
 * tests/scenarios/ compares them (`stored::same`): two decimals by their
 * value, an integer and a decimal never.
 */
export const klopt = (kreeg, verwacht) =>
  JSON.stringify(vergelijkbaar(kreeg)) === JSON.stringify(vergelijkbaar(verwacht));

/**
 * Per answer `step` expects (`step.antwoorden`): the answer, what the cell
 * gave (`kreeg`), whether that is what the scenario expects (`klopt`), and
 * the cell's refusal (`fout`) where it gave none. The cell is asked on the
 * play date of the step, or on `datum` when that is later: the visitor may
 * have played the step on a later day than the scenario gives. `vraag` asks
 * the cell, as `api.antwoord`.
 */
export function toetsAntwoorden(step, datum = null, vraag = api.antwoord) {
  const dag = datum && (!step?.antwoordDatum || datum > step.antwoordDatum) ? datum : step?.antwoordDatum;
  return Promise.all(
    (step?.antwoorden ?? []).map(async (a) => {
      try {
        const gegeven = await vraag(a.cel ?? null, a.bron, a.invoer ?? {}, dag);
        return { ...a, kreeg: gegeven.waarde, klopt: klopt(gegeven.waarde, a.verwacht) };
      } catch (e) {
        return { ...a, kreeg: null, klopt: false, fout: e?.message ?? String(e) };
      }
    }),
  );
}
