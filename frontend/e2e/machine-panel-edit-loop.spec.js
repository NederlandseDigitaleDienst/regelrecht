/**
 * Structured (Machine-panel) edit → re-execute loop end-to-end.
 *
 * Sister spec to `edit-test-loop.spec.js`. Same red → green flow on the
 * zorgtoeslag *Minderjarige* scenario, but the edit that flips the result is
 * driven through the structured operation editor (ActionSheet /
 * OperationSettings) instead of the YAML pane.
 *
 * Why the setup is seeded via YAML rather than typed into the form:
 *  - The structured editor no longer offers an "add condition" affordance on a
 *    logical operation (AND/OR): `canAddValue` is false for logical ops, so a
 *    fresh condition can only be introduced through YAML.
 *  - A cross-law input whose source needs a *literal* peildatum date can't be
 *    bound through the EditSheet's dropdown-only source-parameter controls.
 * So the leeftijd input and a placeholder age condition (threshold 0, which a
 * minor still passes → scenario stays red) are seeded through YAML, and the
 * red → green flip is then performed entirely through the structured
 * ActionSheet: navigate into the age condition and change its threshold to 18.
 * This locks in the form-driven edit → re-execute propagation chain.
 *
 * Requires the WASM engine to be built (frontend/public/wasm/pkg).
 */
import { test, expect } from '@playwright/test';
import * as yaml from 'js-yaml';
import { loadCorpus, loadScenario, mockCorpusApi } from './helpers-corpus.js';
import {
  gotoEditor,
  readYamlSource,
  setYamlPane,
  openSheet,
  openActionEditor,
  saveActionSheet,
  expectScenarioResult,
  clearOpenTabsStorage,
} from './helpers.js';

const MINOR = 'Minderjarige heeft geen recht';

test.describe('Edit → re-execute loop via the structured operation editor', () => {
  test.beforeEach(async ({ page }) => {
    await clearOpenTabsStorage(page);
  });

  test('editing the age threshold via the ActionSheet turns the scenario green', async ({ page }) => {
    // Full-corpus dependency loading + 13 scenarios executing twice needs well
    // over the default 30s per-test budget.
    test.setTimeout(180_000);

    const corpus = loadCorpus();
    const zorgtoeslag = corpus.get('wet_op_de_zorgtoeslag');
    expect(zorgtoeslag).toBeTruthy();

    const scenarioFilename = 'eligibility.feature';
    const scenarioText = loadScenario(zorgtoeslag.path, scenarioFilename);
    expect(scenarioText).toBeTruthy();

    await mockCorpusApi(
      page,
      corpus,
      { id: 'wet_op_de_zorgtoeslag', scenarioFilename },
      scenarioText,
    );

    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');

    // Seed the leeftijd input + a placeholder age condition (threshold 0) into
    // article 2's machine_readable via the YAML pane. The minor still passes
    // age >= 0, so the scenario stays red until we tighten the threshold.
    const originalYaml = await readYamlSource(page);
    expect(originalYaml).toContain('heeft_recht_op_zorgtoeslag');
    const mr = yaml.load(originalYaml);

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

    const heeftRecht = mr.execution.actions.find(
      (a) => a.output === 'heeft_recht_op_zorgtoeslag',
    );
    expect(heeftRecht?.value?.operation).toBe('AND');
    expect(Array.isArray(heeftRecht.value.conditions)).toBe(true);
    heeftRecht.value.conditions.push({
      operation: 'GREATER_THAN_OR_EQUAL',
      subject: '$leeftijd',
      value: 0,
    });
    const seededConditionIndex = heeftRecht.value.conditions.length - 1;

    await setYamlPane(page, yaml.dump(mr, { lineWidth: 80, noRefs: true }));
    await page.waitForTimeout(1000);

    // Red: a minor passes age >= 0, so the AND is still true and the scenario
    // (which expects no entitlement) fails.
    await expectScenarioResult(page, MINOR, 'Mislukt');

    // --- Structured edit: open the action and tighten the age threshold ---
    await openActionEditor(page, 'heeft_recht_op_zorgtoeslag');
    // With the result sheet closed, the ActionSheet is the only visible sheet.
    const sheet = openSheet(page);

    // The sheet opens on the root AND; each condition is a nested-operation
    // value row. Drill into the seeded age condition via its edit pencil (the
    // icon-button's `icon` prop isn't reflected to an attribute, so target its
    // stable `text` label).
    await sheet
      .locator(`[data-testid="op-value-${seededConditionIndex}"] nldd-icon-button[text="Bewerken"]`)
      .first()
      .evaluate((el) => el.click());
    await page.waitForTimeout(200);

    // OperationSettings now shows the comparison's Onderwerp (op-value-0) and
    // Waarde (op-value-1). Change the threshold literal from 0 to 18.
    const waarde = sheet.locator('[data-testid="op-value-1"] nldd-text-field input');
    await waarde.evaluate((el) => {
      el.value = '18';
      el.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await page.waitForTimeout(100);

    await saveActionSheet(page);
    await expect(openSheet(page)).toHaveCount(0);

    // Green: the minor now fails age >= 18, so the AND is false, entitlement is
    // false, and the scenario passes after re-execution.
    await page.waitForTimeout(1000);
    await expectScenarioResult(page, MINOR, 'Geslaagd');
  });
});
