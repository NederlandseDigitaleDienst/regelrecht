/**
 * How far the visitor is in a scenario, read from the chronicles as the app
 * holds them after each step (state.js). A guide (scenarioGuides.js) says per
 * step when it is done, as a test on the counts below, and optionally what the
 * scenario expects of it. The page keeps no record of which button was
 * clicked: what counts is what the cell recorded. Two things are not in a
 * chronicle: checking admissibility (it stays on the page until the decision
 * on the objection, so a guide folds it into that step), and a step the cell
 * refuses, which `state.refusals` keeps (state.js).
 */
import { isMededeling, isOntvangst } from './beroep.js';
import {
  aanvragen,
  besluitOp,
  bezwaarBasis,
  isBesluit,
  state,
  toegewezen,
} from './state.js';

/** What the chronicles hold, counted the way the guides ask for it. */
export function counts() {
  const beslissingen = state.grammen.filter(
    (g) => isBesluit(g) && g.basis === bezwaarBasis(),
  );
  const uit = (g) => g.outputs ?? {};
  const andere = state.andere.flatMap((a) => a.grammen);
  return {
    aanvragen: aanvragen().length,
    // Per application that has a decision, in order: whether it grants.
    decided: aanvragen()
      .map((a) => besluitOp(a.id))
      .filter(Boolean)
      .map((g) => !!toegewezen(g)),
    bezwaren: state.grammen.filter((g) => g.kind === 'BEZWAAR').length,
    beslissingen: beslissingen.length,
    gegrond: beslissingen.filter((g) => uit(g).bestreden_besluit_herroepen === true).length,
    ongegrond: beslissingen.filter(
      (g) => uit(g).bezwaar_ontvankelijk !== false && uit(g).bestreden_besluit_herroepen === false,
    ).length,
    nietOntvankelijk: beslissingen.filter((g) => uit(g).bezwaar_ontvankelijk === false).length,
    // A decision on an objection taken after a judgment was received.
    redecisions: beslissingen.filter((g) => g.after).length,
    beroepen: state.rechter.filter((g) => g.kind === 'BEROEP').length,
    uitspraken: state.rechter.filter((g) => g.kind === 'UITSPRAAK').length,
    ontvangsten: state.grammen.filter(isOntvangst).length,
    // What the authority told the court after an interim judgment (Awb 8:51b).
    mededelingen: state.rechter.filter(isMededeling).length,
    /** Executograms of fact `fact` in the other cells whose fields match `velden`. */
    executogrammen: (fact, velden = {}) =>
      andere.filter(
        (g) =>
          g.kind === 'EXECUTOGRAM' &&
          g.fact === fact &&
          Object.entries(velden).every(([k, v]) => g.fields?.[k] === v),
      ).length,
    refusals: state.refusals,
  };
}

/** A `done` test: the cell refused a step with a message matching `re`. */
export const refused = (re) => (k) => k.refusals.some((m) => re.test(m));

/**
 * Where the visitor is in `guide`, given the counts `k`: the steps done (one
 * flag per step), the index of the first step not done, whether every step
 * is done, and the first expectation a done step does not meet (its text), if
 * any.
 */
export function progress(guide, k) {
  const done = guide.steps.map((s) => s.done(k));
  const current = done.indexOf(false);
  const missed = guide.steps.find((s, i) => done[i] && s.expect && !s.expect.check(k));
  return {
    done,
    current: current === -1 ? guide.steps.length : current,
    finished: current === -1,
    warning: missed?.expect.text ?? null,
  };
}
