import { test, expect } from '@playwright/test';
import { interceptLaw, gotoEditor, selectArticle, readYamlPane, waitForSheet, fillSheetTextField, fillSheetNumberField, saveSheet, pane } from './helpers.js';

test.describe('Definitions', () => {
  test.beforeEach(async ({ page }) => {
    await interceptLaw(page, 'wet_op_de_zorgtoeslag', 'zorgtoeslag-stripped.yaml');
    await gotoEditor(page);
  });

  test('add a numeric definition to article 1a', async ({ page }) => {
    await selectArticle(page, '1a');

    // Init machine_readable
    await pane(page, 'machine').locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Click "Nieuwe definitie" button
    await page.locator('[data-testid="add-def-btn"]').click();
    await waitForSheet(page);

    // Set name
    await fillSheetTextField(page, 'Naam', 'verantwoordelijke_autoriteit');

    // Set value (new definitions default to number)
    await fillSheetNumberField(page, 'Waarde', 42);

    // Save
    await saveSheet(page);

    // Verify the definition appears in the machine readable view
    const mrPane = page.locator('[data-testid="machine-readable"]');
    await expect(mrPane).toContainText('verantwoordelijke_autoriteit');

    // Verify YAML
    const yaml = await readYamlPane(page);
    expect(yaml.definitions).toHaveProperty('verantwoordelijke_autoriteit');
    // A new definition starts structured, as a number (RFC-023)
    expect(yaml.definitions.verantwoordelijke_autoriteit).toEqual({ value: 42, type: 'number' });
  });

  test('add numeric definitions to article 2', async ({ page }) => {
    await selectArticle(page, '2');

    // Init machine_readable
    await pane(page, 'machine').locator('[data-testid="init-mr-btn"]').click();
    await page.waitForTimeout(300);

    // Add definition: drempelinkomen_alleenstaande = 3971900
    await page.locator('[data-testid="add-def-btn"]').click();
    await waitForSheet(page);

    await fillSheetTextField(page, 'Naam', 'drempelinkomen_alleenstaande');
    await fillSheetNumberField(page, 'Waarde', 3971900);
    await saveSheet(page);

    // Verify definition appears
    const mrPane = page.locator('[data-testid="machine-readable"]');
    await expect(mrPane).toContainText('drempelinkomen_alleenstaande');

    // Verify YAML
    const yaml = await readYamlPane(page);
    expect(yaml.definitions.drempelinkomen_alleenstaande).toEqual({ value: 3971900, type: 'number' });
  });
});
