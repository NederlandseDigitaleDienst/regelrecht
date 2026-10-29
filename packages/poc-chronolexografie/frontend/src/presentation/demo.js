/**
 * The scenarios to play right after the deck, all of the parking card case.
 * The choice screen lists them under "Voor de demo". Each one names a
 * scenario of the case YAML, whose guide scenarioGuides.js builds;
 * tests/presentation.test.js keeps them in step.
 *
 * - A card on the advice of the GGD: the happy flow, one application and a
 *   decision that lands in the chronicle.
 * - A second card next to a valid one: the reduction finds the first card in
 *   the chronicle.
 * - A card on objection: the reduction of the Awb passes on the decision that
 *   the decision on objection put in place of the revoked refusal, so at the
 *   next application the municipality finds a valid card it did not have
 *   before the objection.
 */
import { load } from 'js-yaml';
import yaml from '../../../corpus/casussen/gehandicaptenparkeerkaart.yaml?raw';
import { guideFor } from '../scenarioGuides.js';

export const DEMO_CASUS = 'gehandicaptenparkeerkaart';

export const DEMO_SCENARIOS = Object.freeze([
  'De kaart wordt verstrekt op het advies van de GGD',
  'Een tweede kaart naast een geldige',
  'Een kaart na bezwaar telt bij de volgende aanvraag',
]);

// The case as its YAML has it: the choice screen and the deck show these
// scenarios before any case is loaded.
const casus = { id: DEMO_CASUS, ...load(yaml) };

/** The demo scenarios with their guide, in order. */
export const demoGuides = () => DEMO_SCENARIOS.map((naam) => guideFor(casus, naam));
