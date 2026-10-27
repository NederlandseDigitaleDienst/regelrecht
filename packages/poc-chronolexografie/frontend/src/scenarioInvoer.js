/**
 * What the forms are filled in with. While the visitor follows a scenario,
 * the form of the step they are at starts from the case's example values
 * for its kind (`voorbeeld`) with the step's own values over them
 * (`invoer`, casussen/<id>.yaml); the guide names the same values
 * (scenarioGuides.js). In free play every form starts empty. A case from a
 * source has no guide: there a form starts from the example values alone.
 *
 * The visitor can change every value. What they change is theirs until the
 * scenario moves on to its next step; then the form starts again from that
 * step ([`formulier`]).
 */
import { computed, reactive, watch } from 'vue';
import { guideFor } from './scenarioGuides.js';
import { counts, progress } from './scenarioProgress.js';
import { aanvragen, besluitOp, bezwaarBasis, isBesluit, state } from './state.js';

// Which example values a kind of step starts from.
const VOORBEELD = {
  aanvraag: 'aanvraag',
  besluit: 'bevindingen',
  bezwaar: 'bezwaar',
  beroep: 'beroep',
  uitspraak: 'uitspraak',
};

// The step of the scenario the visitor is at, once for every form.
const nu = computed(() => {
  const guide = guideFor(state.casus, state.scenario);
  if (!guide) return null;
  const p = progress(guide, counts());
  return p.finished ? null : { index: p.current, step: guide.steps[p.current] };
});

/** The step of the scenario the visitor is at, `{ index, step }`, or null. */
export const huidigeStap = () => nu.value;

const bezwaren = () => state.grammen.filter((g) => g.kind === 'BEZWAAR');
const beslissingen = () => state.grammen.filter((g) => isBesluit(g) && g.basis === bezwaarBasis());

/**
 * Whether `step` acts on `doel`: the application a decision decides, the
 * decision an objection contests, the objection a decision on objection
 * decides, the cell and fact a record is of. A step of another kind acts
 * on whatever its form shows.
 */
function past(step, doel) {
  switch (step.stap.stap) {
    case 'besluit':
      return !doel.aanvraag || aanvragen()[step.doel - 1]?.id === doel.aanvraag;
    case 'bezwaar': {
      const a = aanvragen()[step.doel - 1];
      return !doel.besluit || (!!a && besluitOp(a.id)?.id === doel.besluit);
    }
    case 'beslissing_op_bezwaar':
      return !doel.bezwaar || bezwaren()[step.doel - 1]?.id === doel.bezwaar;
    case 'beroep':
      return !doel.besluit || beslissingen()[step.doel - 1]?.id === doel.besluit;
    case 'vastleggen':
      return step.stap.cel === doel.cel && step.stap.feit === doel.feit;
    default:
      return true;
  }
}

/**
 * The values the form of kind `soort` (a `stap` of the YAML) starts from
 * now, for what it acts on (`doel`), or null when it starts empty: in free
 * play, or when the scenario is at another step.
 */
export function scenarioWaarden(soort, doel = {}) {
  if (!state.casus || !state.scenario) return null;
  const voorbeeld = state.casus.voorbeeld?.[VOORBEELD[soort]] ?? {};
  if (!guideFor(state.casus, state.scenario)) return VOORBEELD[soort] ? { ...voorbeeld } : null;
  const nu = huidigeStap();
  if (!nu || nu.step.stap.stap !== soort || !past(nu.step, doel)) return null;
  return { ...voorbeeld, ...(nu.step.stap.invoer ?? {}) };
}

/** Whether a form of `soort` starts from example values of the case. */
export const heeftVoorbeeld = (soort) =>
  Object.keys(state.casus?.voorbeeld?.[VOORBEELD[soort]] ?? {}).length > 0;

/** Whether the form of `soort` starts from the scenario with any value, for the hint. */
export const ingevuld = (soort, doel = {}) =>
  Object.keys(scenarioWaarden(soort, doel) ?? {}).length > 0;

/** The names of the values the current step itself gives `soort`, for the hint. */
export function uitScenario(soort, doel = {}) {
  const nu = huidigeStap();
  if (!nu || nu.step.stap.stap !== soort || !past(nu.step, doel)) return [];
  return Object.keys(nu.step.stap.invoer ?? {});
}

/**
 * A set of forms, one per key (an application, an objection, a cell's
 * fact), that start from `standaard(key)` and keep what the visitor
 * changes, an emptied value included, until the scenario moves to another
 * step. `eigen` holds the changes, per key.
 */
export function formulier(standaard) {
  const eigen = reactive({});
  const wis = (sleutel) => delete eigen[sleutel];
  watch(
    () => `${state.begonnen}|${state.scenario}|${nu.value?.index}`,
    () => Object.keys(eigen).forEach(wis),
  );
  return {
    eigen,
    waarden(sleutel) {
      const uit = { ...(standaard(sleutel) ?? {}) };
      for (const [naam, waarde] of Object.entries(eigen[sleutel] ?? {})) {
        if (waarde === undefined) delete uit[naam];
        else uit[naam] = waarde;
      }
      return uit;
    },
    zet(sleutel, naam, waarde) {
      (eigen[sleutel] ??= {})[naam] = waarde;
    },
    wis,
  };
}
