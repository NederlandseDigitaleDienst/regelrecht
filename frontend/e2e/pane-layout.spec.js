/**
 * The editor's default pane layout leaves out the *Scenario's* pane: the
 * scenarios' status sits in the Machine pane's toolbar. A layout saved before
 * that loses the pane once, so the change reaches those who had it; a pane
 * added back afterwards stays.
 */
import { test, expect } from '@playwright/test';
import { loadFixture, gotoEditor, clearOpenTabsStorage } from './helpers.js';
import { mockCorpusApi } from './helpers-corpus.js';

const VIEWS_KEY = 'regelrecht-pane-views';
const MOVED_KEY = 'regelrecht-pane-views-scenario-moved';

const FEATURE = `Feature: Basis

  Scenario: Heeft recht
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
`;

const storedViews = (page) => page.evaluate((key) => JSON.parse(localStorage.getItem(key) ?? 'null'), VIEWS_KEY);
// The view each pane shows, read off its view picker.
const paneViews = (page) =>
  page.locator('nldd-side-by-side-split-view nldd-toolbar-item[slot="start"] > nldd-button[expandable]').evaluateAll((els) => els.map((el) => el.text));

test.describe('Editor pane layout', () => {
  test.beforeEach(async ({ page }) => {
    await clearOpenTabsStorage(page);
    const corpus = new Map([
      ['wet_op_de_zorgtoeslag', { content: loadFixture('zorgtoeslag-full.yaml'), path: '', pubDate: '2024-12-20' }],
    ]);
    await mockCorpusApi(page, corpus, { id: 'wet_op_de_zorgtoeslag', scenarioFilename: 'basis.feature' }, FEATURE);
  });

  test('starts without the Scenario\'s pane, with the status in the Machine toolbar', async ({ page }) => {
    await page.addInitScript(([views, moved]) => {
      localStorage.removeItem(views);
      localStorage.removeItem(moved);
    }, [VIEWS_KEY, MOVED_KEY]);
    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');
    await expect.poll(() => paneViews(page)).toEqual(['Tekst', 'Machine', 'YAML', 'Notities']);
    await expect(page.locator('[data-testid="scenario-status-btn"]')).toHaveCount(1, { timeout: 30_000 });
  });

  test('drops the pane from a layout saved before, once', async ({ page }) => {
    // Seed the old layout on the first load only, as a browser that had it.
    await page.addInitScript(([views, moved]) => {
      if (sessionStorage.getItem('seeded')) return;
      sessionStorage.setItem('seeded', '1');
      localStorage.setItem(views, JSON.stringify(['text', 'scenario', 'yaml']));
      localStorage.removeItem(moved);
    }, [VIEWS_KEY, MOVED_KEY]);
    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');
    await expect.poll(() => paneViews(page)).toEqual(['Tekst', 'YAML']);
    expect(await storedViews(page)).toEqual(['text', 'yaml']);

    // Added back, as the pane picker would: it stays on the next load.
    await page.evaluate((key) => localStorage.setItem(key, JSON.stringify(['text', 'scenario'])), VIEWS_KEY);
    await page.reload();
    await page.waitForSelector('nldd-document-tab-bar-item', { timeout: 10_000 });
    await expect.poll(() => paneViews(page)).toEqual(['Tekst', "Scenario's"]);
  });
});
