/**
 * A scenario on a date that another version of the open regelwerk covers runs
 * on that version. The editor loads the regelwerk's saved versions next to the
 * one being edited; before, it loaded only the open one, and a 2024 scenario of
 * the zorgtoeslag failed with "No version of law ... in force on 2024-01-01".
 */
import { test, expect } from '@playwright/test';
import * as yaml from 'js-yaml';
import { loadCorpus, loadScenario, mockCorpusApi } from './helpers-corpus.js';
import { gotoEditor, clearOpenTabsStorage, expectScenarioResult, readYamlSource, setYamlPane } from './helpers.js';

async function openZorgtoeslag(page) {
  await clearOpenTabsStorage(page);
  const corpus = loadCorpus();
  const zorgtoeslag = corpus.get('wet_op_de_zorgtoeslag');
  await mockCorpusApi(
    page,
    corpus,
    { id: 'wet_op_de_zorgtoeslag', scenarioFilename: 'eligibility.feature' },
    loadScenario(zorgtoeslag.path, 'eligibility.feature'),
  );
  await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');
}

test('a 2024 scenario runs on the 2024 version of the open regelwerk', async ({ page }) => {
  // Full-corpus dependency loading takes longer than the default budget.
  test.setTimeout(120_000);
  await openZorgtoeslag(page);
  await expectScenarioResult(page, 'Meerderjarige heeft recht op zorgtoeslag (2024)', 'Geslaagd');
  await expectScenarioResult(page, 'Partner met gezamenlijk inkomen en vermogen boven de grens (2024)', 'Geslaagd');
});

test('an edit the engine refuses fails the scenarios, rather than running them on a saved version', async ({ page }) => {
  test.setTimeout(120_000);
  await openZorgtoeslag(page);
  await expectScenarioResult(page, 'Meerderjarige heeft recht op zorgtoeslag (2024)', 'Geslaagd');

  // Valid YAML with an operation the engine does not know: it refuses the
  // edited version, and the regelwerk leaves the engine.
  const mr = yaml.load(await readYamlSource(page));
  const action = mr.execution.actions.find((a) => a.output === 'heeft_recht_op_zorgtoeslag');
  expect(action, 'heeft_recht_op_zorgtoeslag action must exist').toBeTruthy();
  action.value = { operation: 'BESTAAT_NIET' };
  await setYamlPane(page, yaml.dump(mr, { lineWidth: 80, noRefs: true }));

  // Every scenario of the regelwerk fails, the 2024 one too.
  await expectScenarioResult(page, 'Meerderjarige heeft recht op zorgtoeslag (2024)', 'Mislukt');
  await expectScenarioResult(page, 'Meerderjarige met actieve polis heeft recht op zorgtoeslag', 'Mislukt');
});
