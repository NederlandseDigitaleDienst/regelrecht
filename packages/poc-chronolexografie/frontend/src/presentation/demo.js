/**
 * The demo scenarios, all of the parking card case, from simple to
 * involved. The choice screen lists them under the links to the deck and the
 * explanation page. Each one names a scenario of the case YAML, whose guide
 * scenarioGuides.js builds; tests/presentation.test.js keeps them in step.
 *
 * - A card on the advice of the GGD: the happy flow; the decision reads the
 *   examination and the advice from the GGD's own chronicle.
 * - An objection and a new advice: the reconsideration reads the chronicle as
 *   it stands on the day of the decision on objection, so the new advice
 *   counts.
 * - A new card five years later: the municipality finds its own decision of
 *   2026 in its chronicle, so no new examination is needed.
 */
import { load } from 'js-yaml';
import yaml from '../../../corpus/casussen/gehandicaptenparkeerkaart.yaml?raw';
import { guideFor } from '../scenarioGuides.js';

export const DEMO_CASUS = 'gehandicaptenparkeerkaart';

export const DEMO_SCENARIOS = Object.freeze([
  'De kaart op advies van de GGD',
  'Bezwaar, en een nieuw advies',
  'Vijf jaar later een nieuwe kaart',
]);

// The case as its YAML has it: the choice screen and the deck show these
// scenarios before any case is loaded.
const casus = { id: DEMO_CASUS, ...load(yaml) };

/** The demo scenarios with their guide, in order. */
export const demoGuides = () => DEMO_SCENARIOS.map((naam) => guideFor(casus, naam));
