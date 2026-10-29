import { test, expect } from '@playwright/test';
import {
  interceptLaw,
  gotoEditor,
  selectArticle,
  readYamlPane,
  waitForSheet,
  fillSheetNumberField,
  fillSheetTextField,
  saveSheet,
  openSheet,
} from './helpers.js';

// Structured definitions (RFC-023): `{ value, type, type_spec }`. The fixture's
// article 2 holds one of every shape the edit sheet offers, plus a bare value.

async function editDefinition(page, name) {
  await page.locator(`[data-testid="def-${name}-edit-btn"]`).evaluate((el) => el.click());
  await waitForSheet(page);
}

async function choose(page, testid, value) {
  await openSheet(page)
    .locator(`select[data-testid="${testid}"]`)
    .evaluate((el, v) => {
      el.value = v;
      el.dispatchEvent(new Event('change', { bubbles: true }));
    }, value);
  await page.waitForTimeout(100);
}

async function definitions(page) {
  const yaml = await readYamlPane(page);
  return yaml.articles?.find((a) => String(a.number) === '2')?.machine_readable?.definitions ?? yaml.definitions;
}

test.describe('Structured definitions', () => {
  test.beforeEach(async ({ page }) => {
    await interceptLaw(page, 'wet_op_de_zorgtoeslag', 'zorgtoeslag-structured.yaml');
    await gotoEditor(page);
    await selectArticle(page, '2');
  });

  test('a percentage is edited as the number it is', async ({ page }) => {
    await editDefinition(page, 'percentage_toetsingsinkomen');
    await expect(openSheet(page).locator('[data-testid="def-suffix"]')).toHaveAttribute('text', '%');
    await fillSheetNumberField(page, 'Waarde', 13.73);
    await saveSheet(page);

    expect((await definitions(page)).percentage_toetsingsinkomen).toEqual({
      value: 13.73,
      type: 'number',
      type_spec: { unit: 'percentage' },
    });
  });

  test('a bare value gets a type and a unit', async ({ page }) => {
    await editDefinition(page, 'percentage_drempelinkomen_partner');
    await choose(page, 'def-type', 'number');
    await choose(page, 'def-unit', 'ratio');
    // A ratio stays 0-1 and says what it amounts to.
    await expect(openSheet(page).locator('[data-testid="def-suffix"]')).toHaveAttribute('text', '= 4,273%');
    await saveSheet(page);

    expect((await definitions(page)).percentage_drempelinkomen_partner).toEqual({
      value: 0.04273,
      type: 'number',
      type_spec: { unit: 'ratio' },
    });
  });

  // Renaming marks the sheet changed without touching the value, so the value
  // has to come through the form and back exactly as it was.
  test('renaming keeps the value, whatever its shape', async ({ page }) => {
    const names = ['drempelinkomen_alleenstaande', 'percentage_drempelinkomen_partner', 'geldige_partnertypen', 'datum_inwerkingtreding'];
    const before = await definitions(page);
    for (const name of names) {
      await editDefinition(page, name);
      await fillSheetTextField(page, 'Naam', `${name}_x`);
      await saveSheet(page);
    }
    const after = await definitions(page);
    for (const name of names) {
      expect(after[`${name}_x`]).toEqual(before[name]);
    }
  });

  test('an amount is typed in euros and stored in eurocent', async ({ page }) => {
    await editDefinition(page, 'drempelinkomen_alleenstaande');
    await fillSheetNumberField(page, 'Waarde', 40000.5);
    await saveSheet(page);

    expect((await definitions(page)).drempelinkomen_alleenstaande).toEqual({
      value: 4000050,
      type: 'amount',
      type_spec: { unit: 'eurocent' },
    });
  });
});
