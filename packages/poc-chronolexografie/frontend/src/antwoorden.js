/**
 * The answers a scenario expects of a cell after a step (`antwoorden` in the
 * case YAML), such as what a payment leaves outstanding: an answer of a
 * reduction, which no decision holds. The guide asks the cell for each one
 * on the play date of the step, as tests/scenarios/ does, and shows what it
 * answered beside what the scenario expects (ScenarioGuide.vue). Nothing is
 * recorded.
 */
import { api } from './api.js';

/** Whether the cell's answer `kreeg` is the value the scenario expects. */
export const klopt = (kreeg, verwacht) => JSON.stringify(kreeg) === JSON.stringify(verwacht);

/**
 * Per answer `step` expects (`step.antwoorden`): the answer, what the cell
 * gave (`kreeg`), whether that is what the scenario expects (`klopt`), and
 * the cell's refusal (`fout`) where it gave none. `vraag` asks the cell, as
 * `api.antwoord`.
 */
export function toetsAntwoorden(step, vraag = api.antwoord) {
  return Promise.all(
    (step?.antwoorden ?? []).map(async (a) => {
      try {
        const gegeven = await vraag(a.cel ?? null, a.bron, a.invoer ?? {}, step.antwoordDatum);
        return { ...a, kreeg: gegeven.waarde, klopt: klopt(gegeven.waarde, a.verwacht) };
      } catch (e) {
        return { ...a, kreeg: null, klopt: false, fout: e?.message ?? String(e) };
      }
    }),
  );
}
