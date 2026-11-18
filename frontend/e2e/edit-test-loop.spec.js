/**
 * Edit → re-execute loop end-to-end.
 *
 * Verifies the demo flow the editor was built for:
 *   1. Open wet_op_de_zorgtoeslag in the editor.
 *   2. Observe the *Minderjarige heeft geen recht op zorgtoeslag* scenario is
 *      failing (the read-only scenario pane tags it "Mislukt") - the age check
 *      isn't in the law's machine_readable, so a minor still qualifies.
 *   3. Edit article 2's machine_readable via the YAML pane to add a leeftijd
 *      input + an age condition to the existing AND, without saving.
 *   4. The scenario pane runs again against the edited YAML.
 *   5. Observe the scenario now passes ("Geslaagd").
 *
 * This is the smoke-test for the propagation chain:
 *   machineReadable edit → currentLawYaml computed → engine reload →
 *   scenario pane lawYaml prop → dependency reload → run.
 *
 * All corpus laws, the scenarios list, the scenario feature file, and the
 * PUT save endpoint are mocked from the on-disk corpus directory so the
 * spec doesn't need a running editor-api. Requires the WASM engine to be
 * built (frontend/public/wasm/pkg, via `just wasm-build`).
 */
import { test, expect } from '@playwright/test';
import * as yaml from 'js-yaml';
import { loadCorpus, loadScenario, mockCorpusApi } from './helpers-corpus.js';
import { gotoEditor, readYamlSource, setYamlPane, expectScenarioResult, clearOpenTabsStorage } from './helpers.js';

const MINOR = 'Minderjarige heeft geen recht';

test.describe('Edit → re-execute loop', () => {
  test.beforeEach(async ({ page }) => {
    // Clear the per-traject open-tab storage (by prefix) to avoid bleed-over.
    await clearOpenTabsStorage(page);
  });

  test('Minderjarige scenario goes red → green after adding age check', async ({ page }) => {
    // Full-corpus dependency loading + 13 scenarios executing twice (before and
    // after the edit) needs well over the default 30s per-test budget.
    test.setTimeout(180_000);
    const corpus = loadCorpus();
    const zorgtoeslag = corpus.get('wet_op_de_zorgtoeslag');
    expect(zorgtoeslag, 'wet_op_de_zorgtoeslag must exist in the test corpus').toBeTruthy();

    const scenarioFilename = 'eligibility.feature';
    const scenarioText = loadScenario(zorgtoeslag.path, scenarioFilename);
    expect(scenarioText, 'eligibility.feature must exist').toBeTruthy();

    await mockCorpusApi(
      page,
      corpus,
      { id: 'wet_op_de_zorgtoeslag', scenarioFilename },
      scenarioText,
    );

    // Navigate directly to article 2 via the route param - that's where
    // heeft_recht_op_zorgtoeslag lives and where we need to edit.
    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');

    // Initial state: the scenario is failing (age check not in the law), so
    // the pane tags it "Mislukt".
    await expectScenarioResult(page, MINOR, 'Mislukt');

    // Grab the current YAML (article 2's machine_readable), parse it,
    // surgically add the leeftijd input and the age condition, and write it
    // back into the code editor.
    const originalYaml = await readYamlSource(page);
    expect(originalYaml).toContain('heeft_recht_op_zorgtoeslag');
    const mr = yaml.load(originalYaml);

    // Inject leeftijd input (sourced from BRP with a literal peildatum).
    // A literal date is intentional for test isolation - the spec must remain
    // stable regardless of the real calculation date at CI time.
    mr.execution.input.push({
      name: 'leeftijd',
      type: 'number',
      source: {
        regulation: 'wet_basisregistratie_personen',
        output: 'leeftijd',
        parameters: {
          bsn: '$bsn',
          peildatum: '2025-01-01',
        },
      },
    });

    // Append the age condition to the heeft_recht_op_zorgtoeslag AND.
    const heeftRecht = mr.execution.actions.find(
      (a) => a.output === 'heeft_recht_op_zorgtoeslag',
    );
    expect(heeftRecht, 'heeft_recht_op_zorgtoeslag action must exist').toBeTruthy();
    expect(
      heeftRecht.value?.operation,
      'heeft_recht action must be an AND at the top level',
    ).toBe('AND');
    expect(Array.isArray(heeftRecht.value?.conditions)).toBe(true);
    heeftRecht.value.conditions.push({
      operation: 'GREATER_THAN_OR_EQUAL',
      subject: '$leeftijd',
      value: 18,
    });

    const editedYaml = yaml.dump(mr, { lineWidth: 80, noRefs: true });

    // Write the edited YAML through the code editor; the editor's onYamlInput
    // handler parses it and updates machineReadable, which re-executes the
    // scenarios against the edited law.
    await setYamlPane(page, editedYaml);

    // Once the engine, the dependencies and the run have caught up, the minor
    // fails the age check, so the AND is false and the scenario passes.
    await expectScenarioResult(page, MINOR, 'Geslaagd');
  });
});
