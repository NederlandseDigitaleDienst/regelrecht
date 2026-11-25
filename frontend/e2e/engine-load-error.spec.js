/**
 * A regelwerk the engine refuses. Opened that way, the error takes the place
 * of the panes, as a readable message with the engine's own text one click
 * away. Broken by an unsaved edit, the panes stay and the error is a
 * notification, so the mistake can be repaired where it was made.
 *
 * Requires the WASM engine to be built (frontend/public/wasm/pkg).
 */
import { test, expect } from '@playwright/test';
import { loadFixture, gotoEditor, clearOpenTabsStorage, setYamlPane, readYamlSource } from './helpers.js';
import { mockCorpusApi } from './helpers-corpus.js';

// The frontend's own parser reads this; the engine knows no such operation.
const breakLaw = (yamlText) => yamlText.replace('operation: MAX', 'operation: NO_SUCH_OPERATION');

const splitView = (page) => page.locator('nldd-side-by-side-split-view');
const blockingError = (page) => page.locator('[data-testid="engine-load-error"]');
const notification = (page) => page.locator('[data-testid="error-notification"]');
const detailsSheet = (page) => page.locator('[data-testid="error-details-sheet"]');

async function mockLaw(page, content) {
  await clearOpenTabsStorage(page);
  const corpus = new Map([['wet_op_de_zorgtoeslag', { content, path: '', pubDate: '2024-12-20' }]]);
  await mockCorpusApi(page, corpus, { id: 'wet_op_de_zorgtoeslag', scenarioFilename: 'basis.feature' }, '');
}

test.describe('A regelwerk the engine refuses', () => {
  test('opened broken: the error replaces the panes', async ({ page }) => {
    await mockLaw(page, breakLaw(loadFixture('zorgtoeslag-full.yaml')));
    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');

    await expect(blockingError(page)).toBeVisible({ timeout: 30_000 });
    await expect(splitView(page)).toHaveCount(0);
    // The message is the readable sentence, not the engine's text.
    await expect(blockingError(page)).not.toHaveAttribute('supporting-text', /NO_SUCH_OPERATION/);

    await blockingError(page).locator('nldd-button[text="Technische details"]').click();
    await expect(detailsSheet(page)).toHaveAttribute('open', '');
    await expect(detailsSheet(page).locator('[data-testid="error-details-text"]')).toContainText('NO_SUCH_OPERATION');
  });

  test('opened broken: the panes come back on request, with a notification', async ({ page }) => {
    await mockLaw(page, breakLaw(loadFixture('zorgtoeslag-full.yaml')));
    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');

    await expect(blockingError(page)).toBeVisible({ timeout: 30_000 });
    await page.locator('[data-testid="engine-load-error-show-panes"]').click();
    await expect(splitView(page)).toHaveCount(1);
    await expect(notification(page)).toBeVisible();
  });

  test('broken by an unsaved edit: the panes stay', async ({ page }) => {
    await mockLaw(page, loadFixture('zorgtoeslag-full.yaml'));
    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');
    await expect(splitView(page)).toHaveCount(1);

    await setYamlPane(page, breakLaw(await readYamlSource(page)));
    await expect(notification(page)).toBeVisible({ timeout: 30_000 });
    await expect(splitView(page)).toHaveCount(1);
    await expect(blockingError(page)).toHaveCount(0);
    await expect(notification(page)).toHaveAttribute('text', 'Dit regelwerk kan niet worden uitgevoerd');

    await notification(page).locator('nldd-button').click();
    await expect(detailsSheet(page).locator('[data-testid="error-details-text"]')).toContainText('NO_SUCH_OPERATION');
  });
});
