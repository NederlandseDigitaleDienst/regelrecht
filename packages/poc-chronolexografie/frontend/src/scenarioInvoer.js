/**
 * What the forms are filled in with. While the visitor follows a scenario,
 * the form of the step they are at starts from the case's example values
 * for its kind (`voorbeeld`) with the step's own values over them
 * (`invoer`, casussen/<id>.yaml); the guide names the same values
 * (scenarioGuides.js). In free play every form starts empty. A scenario
 * without a guide (its steps are all text) starts every form from the
 * example values alone.
 *
 * The visitor can change every value. What they change is theirs until the
 * scenario moves on to its next step; then the form starts again from that
 * step ([`formulier`]).
 */
import { computed, reactive, watch } from "vue";
import { guideFor, vastleggendeCel } from "./scenarioGuides.js";
import { counts, progress } from "./scenarioProgress.js";
import {
  aanvragen,
  besluitOp,
  bezwaarBasis,
  isBesluit,
  state,
} from "./state.js";

// Which example values a kind of step starts from.
const VOORBEELD = {
  aanvraag: "aanvraag",
  besluit: "bevindingen",
  bezwaar: "bezwaar",
  // Het oordeel van het bestuursorgaan over de ontvankelijkheid (Awb 7:11).
  beslissing_op_bezwaar: "beslissing_op_bezwaar",
  opnieuw_beslissen: "beslissing_op_bezwaar",
  beroep: "beroep",
  uitspraak: "uitspraak",
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

const bezwaren = () => state.grammen.filter((g) => g.kind === "BEZWAAR");
// The decisions on objection, without those taken again after a judgment.
const beslissingen = () =>
  state.grammen.filter(
    (g) => isBesluit(g) && g.basis === bezwaarBasis() && !g.after,
  );

/**
 * Whether `step` acts on `doel`: the application a decision decides, the
 * decision an objection contests, the objection a decision on objection
 * decides, the cell and fact a record is of. A step of another kind acts
 * on whatever its form shows.
 */
function past(step, doel) {
  switch (step.stap.stap) {
    case "besluit":
      return !doel.aanvraag || aanvragen()[step.doel - 1]?.id === doel.aanvraag;
    case "bezwaar": {
      const a = aanvragen()[step.doel - 1];
      return !doel.besluit || (!!a && besluitOp(a.id)?.id === doel.besluit);
    }
    case "beslissing_op_bezwaar":
      return !doel.bezwaar || bezwaren()[step.doel - 1]?.id === doel.bezwaar;
    case "beroep":
      return (
        !doel.besluit || beslissingen()[step.doel - 1]?.id === doel.besluit
      );
    case "vastleggen":
      return (
        vastleggendeCel(step.stap, state.casus) === doel.cel &&
        step.stap.feit === doel.feit
      );
    case "ambtshalve":
      return !doel.artikel || step.stap.artikel === doel.artikel;
    default:
      return true;
  }
}

/** Whether the scenario is at a step of kind `soort` that acts on `doel`. */
function isHuidigeStap(soort, doel = {}) {
  const nu = huidigeStap();
  return !!nu && nu.step.stap.stap === soort && past(nu.step, doel);
}

/**
 * Whether the form of `feit` of cell `cel` folds away: the scenario is at a
 * record step on another form of `cellen` (`state.vormen.andere_cellen`).
 * At any other step, or in free play, every form stays open.
 */
export function ingeklapt(cellen, cel, feit) {
  if (isHuidigeStap("vastleggen", { cel, feit })) return false;
  return cellen.some((c) =>
    (c.handelingen ?? []).some((h) =>
      isHuidigeStap("vastleggen", { cel: c.cel, feit: h.feit }),
    ),
  );
}

/**
 * The values the form of kind `soort` (a `stap` of the YAML) starts from
 * now, for what it acts on (`doel`), or null when it starts empty: in free
 * play, or when the scenario is at another step.
 */
export function scenarioWaarden(soort, doel = {}) {
  if (!state.casus || !state.scenario) return null;
  const voorbeeld = state.casus.voorbeeld?.[VOORBEELD[soort]] ?? {};
  if (!guideFor(state.casus, state.scenario))
    return VOORBEELD[soort] ? { ...voorbeeld } : null;
  if (!isHuidigeStap(soort, doel)) return null;
  return { ...voorbeeld, ...(huidigeStap().step.stap.invoer ?? {}) };
}

/**
 * The earlier record that the record step the scenario is at refers to
 * (`verwijst_naar`), when that step records `feit` in cell `cel`: the id of
 * the record of an earlier step with that kenmerk, or else the kenmerk as it
 * is, for a record in the begin of the cell. Null otherwise, and always in
 * free play.
 */
export function scenarioVerwijzing(cel, feit) {
  if (!isHuidigeStap("vastleggen", { cel, feit })) return null;
  return verwijzing(huidigeStap().step.stap.verwijst_naar);
}

/**
 * The decision that the ex officio step the scenario is at concerns
 * (`verwijst_naar`), when that step decides under `artikel`: as for a record
 * step ([`scenarioVerwijzing`]). Null otherwise, and always in free play.
 */
export function scenarioOver(artikel) {
  if (!isHuidigeStap("ambtshalve", { artikel })) return null;
  return verwijzing(huidigeStap().step.stap.verwijst_naar);
}

/**
 * The article the form for an ex officio decision acts under: what the
 * visitor chose (`gekozen`), or else, at an ex officio step of a scenario,
 * the step's article when the case's cell can take it (`artikelen`). Never
 * a default of its own: an ex officio decision nobody asked for, taken with
 * one click, is a decision on a law the visitor did not choose. Null until
 * there is a choice.
 */
export function ambtshalveArtikel(
  gekozen,
  artikelen,
  stap = huidigeStap()?.step.stap,
) {
  if (gekozen && artikelen.includes(gekozen)) return gekozen;
  if (stap?.stap === "ambtshalve" && artikelen.includes(stap.artikel))
    return stap.artikel;
  return null;
}

// The id of the record of the earlier step with kenmerk `naar`, or else the
// kenmerk as it is (a record in the begin of the cell); null without one.
function verwijzing(naar) {
  if (!naar) return null;
  const eerder = guideFor(state.casus, state.scenario).steps.find(
    (s) => s.stap.kenmerk === naar,
  );
  return (eerder && recordVan(eerder)) ?? naar;
}

/**
 * The id of the record that the step of the scenario with `kenmerk` made, or
 * null when no step gives it or its record is not there (yet). The guide
 * knows the record of a decision and of a record step (`recordVan`); a
 * scenario of generic steps has no guide (scenarioGuides.js).
 */
export function recordMetKenmerk(kenmerk) {
  const guide = guideFor(state.casus, state.scenario);
  const step = guide?.steps.find((s) => s.stap.kenmerk === kenmerk);
  return step ? recordVan(step) : null;
}

/**
 * The id of the record that the done step `step` made, or null: the decision
 * on its application, or the executogram it recorded, counted among the
 * records of its fact in the chronicle of its cell.
 */
function recordVan(step) {
  if (step.stap.stap === "besluit") {
    const a = aanvragen()[step.doel - 1];
    return (a && besluitOp(a.id)?.id) ?? null;
  }
  if (step.stap.stap !== "vastleggen") return null;
  const cel = vastleggendeCel(step.stap, state.casus);
  const grammen =
    cel === state.casus.cel
      ? state.grammen
      : (state.andere.find((a) => a.cel === cel)?.grammen ?? []);
  const vanFeit = grammen.filter(
    (g) =>
      g.kind === "EXECUTOGRAM" && `${g.basis}#${g.fact}` === step.stap.feit,
  );
  return vanFeit[step.vooraf + step.keer - 1]?.id ?? null;
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
