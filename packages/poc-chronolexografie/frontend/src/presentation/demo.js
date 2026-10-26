/**
 * The scenarios to play right after the deck, all of the parking card case.
 * The choice screen lists them under "Voor de demo" and the closing slide
 * offers them as buttons. Each one names a scenario of the case YAML and its
 * guide (scenarioGuides.js); tests/presentation.test.js keeps them in step.
 *
 * - A second card next to a valid one: the reduction finds the first card in
 *   the chronicle.
 * - A card on objection: the reduction of the Awb passes on the decision that
 *   the decision on objection put in place of the revoked refusal, so at the
 *   next application the municipality finds a valid card it did not have
 *   before the objection.
 */
import { guides } from '../scenarioGuides.js';

export const DEMO_CASUS = 'gehandicaptenparkeerkaart';

export const DEMO_SCENARIOS = Object.freeze([
  'Een tweede kaart naast een geldige',
  'Een kaart na bezwaar telt bij de volgende aanvraag',
]);

/** The demo scenarios with their guide, in order. */
export const demoGuides = () =>
  DEMO_SCENARIOS.map((naam) => guides[DEMO_CASUS].find((g) => g.name === naam));
