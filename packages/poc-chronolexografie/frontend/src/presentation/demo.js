/**
 * The demo scenarios, from simple to involved, over two cases that run in
 * the same environment. The choice screen lists them under the links to the
 * deck and the explanation page. Each one names a case and a scenario of its
 * YAML, whose guide scenarioGuides.js builds; tests/presentation.test.js
 * keeps them in step.
 *
 * - A card on the advice of the GGD: the happy flow; the advice arrives as a
 *   message, and the decision reads it from the municipality's own chronicle.
 * - An objection and a new advice: the reconsideration reads the chronicle as
 *   it stands on the day of the decision on objection, so the new advice
 *   counts.
 * - A second permit as the company grows: the municipality counts its own
 *   earlier permit in its chronicle, and reads the number of people at work
 *   from the chronicle of the Chamber of Commerce as it stands on the day of
 *   the decision.
 */
import { load } from 'js-yaml';
import parkeerkaart from '../../../corpus/casussen/gehandicaptenparkeerkaart.yaml?raw';
import bedrijfsparkeervergunning from '../../../corpus/casussen/bedrijfsparkeervergunning.yaml?raw';
import { guideFor } from '../scenarioGuides.js';

export const DEMO_SCENARIOS = Object.freeze([
  { casus: 'gehandicaptenparkeerkaart', naam: 'De kaart op advies van de GGD' },
  { casus: 'gehandicaptenparkeerkaart', naam: 'Bezwaar, en een nieuw advies' },
  { casus: 'bedrijfsparkeervergunning', naam: 'Een tweede vergunning als het bedrijf groeit' },
]);

// The cases as their YAML has them: the choice screen shows these scenarios
// before any case is loaded.
const YAML = { gehandicaptenparkeerkaart: parkeerkaart, bedrijfsparkeervergunning };
const casussen = Object.fromEntries(Object.entries(YAML).map(([id, y]) => [id, { id, ...load(y) }]));

/** The demo scenarios in order, each its case (`casus`, `titel`) with its guide. */
export const demoGuides = () =>
  DEMO_SCENARIOS.map(({ casus, naam }) => ({
    casus,
    titel: casussen[casus].titel,
    guide: guideFor(casussen[casus], naam),
  }));
