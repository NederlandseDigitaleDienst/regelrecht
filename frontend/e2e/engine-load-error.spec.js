/**
 * A regelwerk the engine refuses. The panes stay, because that is where the
 * mistake is repaired; the error is a notification with a readable message
 * and the engine's own text one click away.
 *
 * Requires the WASM engine to be built (frontend/public/wasm/pkg).
 */
import { test, expect } from '@playwright/test';
import { loadFixture, gotoEditor, clearOpenTabsStorage, setYamlPane, readYamlSource } from './helpers.js';
import { mockCorpusApi } from './helpers-corpus.js';

// The frontend's own parser reads this; the engine knows no such operation.
const breakLaw = (yamlText, operation = 'NO_SUCH_OPERATION') => yamlText.replace('operation: MAX', `operation: ${operation}`);

const splitView = (page) => page.locator('nldd-side-by-side-split-view');
const notification = (page) => page.locator('[data-testid="error-notification"]');
const detailsSheet = (page) => page.locator('[data-testid="error-details-sheet"]');

async function mockLaw(page, content) {
  await clearOpenTabsStorage(page);
  const corpus = new Map([['wet_op_de_zorgtoeslag', { content, path: '', pubDate: '2024-12-20' }]]);
  await mockCorpusApi(page, corpus, { id: 'wet_op_de_zorgtoeslag', scenarioFilename: 'basis.feature' }, '');
}

test.describe('A regelwerk the engine refuses', () => {
  test('opened broken: the panes stay, with the error as a notification', async ({ page }) => {
    await mockLaw(page, breakLaw(loadFixture('zorgtoeslag-full.yaml')));
    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');

    await expect(notification(page)).toBeVisible({ timeout: 30_000 });
    await expect(splitView(page)).toHaveCount(1);
    await expect(notification(page)).toHaveAttribute('text', 'Wet op de zorgtoeslag kan niet worden uitgevoerd');
    // The message is the readable sentence, not the engine's text.
    await expect(notification(page)).not.toHaveAttribute('supporting-text', /NO_SUCH_OPERATION/);
    await expect(notification(page)).toHaveAttribute('supporting-text', /Kijk bij artikel 2\./);

    await notification(page).locator('nldd-button').click();
    await expect(detailsSheet(page)).toHaveAttribute('open', '');
    await expect(detailsSheet(page).locator('[data-testid="error-details-text"]')).toContainText('NO_SUCH_OPERATION');
  });

  test('closing the notification leaves a sign in the top bar that opens the details', async ({ page }) => {
    await mockLaw(page, breakLaw(loadFixture('zorgtoeslag-full.yaml')));
    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');
    const indicator = page.locator('[data-testid="error-indicator"]:visible');

    await expect(notification(page)).toBeVisible({ timeout: 30_000 });
    await expect(indicator).toHaveAttribute('text', '1 fout');

    await notification(page).evaluate((el) => el.dispatchEvent(new CustomEvent('dismiss')));
    await expect(notification(page)).toHaveCount(0);
    await expect(indicator).toBeVisible();

    await indicator.click();
    await expect(detailsSheet(page)).toHaveAttribute('open', '');
    await expect(detailsSheet(page).locator('[data-testid="error-details-text"]')).toContainText('NO_SUCH_OPERATION');

    // Closing the sheet hands the focus back to the sign it was opened from.
    await page.keyboard.press('Escape');
    await expect(detailsSheet(page)).not.toHaveAttribute('open', '');
    await expect(indicator).toBeFocused();
    // The notification was dismissed and stays away.
    await expect(notification(page)).toHaveCount(0);
  });

  test('broken by an unsaved edit: the panes stay', async ({ page }) => {
    await mockLaw(page, loadFixture('zorgtoeslag-full.yaml'));
    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');
    await expect(splitView(page)).toHaveCount(1);

    await setYamlPane(page, breakLaw(await readYamlSource(page)));
    await expect(notification(page)).toBeVisible({ timeout: 30_000 });
    await expect(splitView(page)).toHaveCount(1);
    await expect(notification(page)).toHaveAttribute('text', 'Wet op de zorgtoeslag kan niet worden uitgevoerd');

    await expect(notification(page)).toHaveAttribute('supporting-text', /Kijk bij artikel 2\./);

    await notification(page).locator('nldd-button').click();
    await expect(detailsSheet(page).locator('[data-testid="error-details-text"]')).toContainText('NO_SUCH_OPERATION');
  });

  test('a dismissed notification stays away until the error changes', async ({ page }) => {
    const saved = loadFixture('zorgtoeslag-full.yaml');
    await mockLaw(page, saved);
    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');
    const source = await readYamlSource(page);

    await setYamlPane(page, breakLaw(source));
    await expect(notification(page)).toBeVisible({ timeout: 30_000 });
    await notification(page).dispatchEvent('dismiss');
    await expect(notification(page)).toHaveCount(0);

    // Another edit, the same refusal: the engine reloads (300 ms debounce)
    // and the notification does not come back.
    await setYamlPane(page, `${breakLaw(source)}\n`);
    await page.waitForTimeout(1_500);
    await expect(notification(page)).toHaveCount(0);

    // A different refusal is a new error.
    await setYamlPane(page, breakLaw(source, 'ANOTHER_UNKNOWN_OPERATION'));
    await expect(notification(page)).toBeVisible({ timeout: 30_000 });
  });
});
