/**
 * The bare editor root within a traject: going there with tabs open restores
 * the last article, and closing the last tab lands there, responsive.
 *
 * Regression: closing the last tab navigated to the editor root, and the route
 * guard for that navigation restored the traject's tabs before it committed.
 * With no tab left the restore navigated to the root again, from inside the
 * guard, so every navigation started the next one and the page hung.
 *
 * Requires the WASM engine to be built (frontend/public/wasm/pkg); the editor
 * mounts it but scenario execution is irrelevant here.
 */
import { test, expect } from '@playwright/test';
import { loadCorpus, loadScenario, mockCorpusApi } from './helpers-corpus.js';
import { TEST_TRAJECT_REF } from './helpers.js';

const tabItems = (page) => page.locator('nldd-document-tab-bar-item');
// The close button of the active tab: closing it is what selects a replacement.
const closeButton = (page) =>
  page.locator('nldd-document-tab-bar-item[selected] .document-tab-bar__item-dismiss-button');
const activeTab = (page) => page.locator('nldd-document-tab-bar-item[selected]');

// Navigate in-app through the app's router, as the Editor tab or browser back
// does; resolves to whether the navigation went through.
async function navigateInApp(page, path) {
  return page.evaluate(async (target) => {
    const app = document.querySelector('#app').__vue_app__;
    const failure = await app.config.globalProperties.$router.push(target);
    return !failure;
  }, path);
}
const neutralState = (page) =>
  page.getByText('Open een artikel vanuit de tabbalk of Home', { exact: false });

// Whether the page can still run a script within `ms`; a hung page cannot.
async function answers(page, ms = 5_000) {
  const probe = page.evaluate(() => true);
  const timeout = new Promise((resolve) => setTimeout(() => resolve(false), ms));
  return Promise.race([probe, timeout]);
}

test.describe('The bare editor root', () => {
  test.beforeEach(async ({ page }) => {
    const corpus = loadCorpus();
    const zorgtoeslag = corpus.get('wet_op_de_zorgtoeslag');
    await mockCorpusApi(
      page,
      corpus,
      { id: 'wet_op_de_zorgtoeslag', scenarioFilename: 'eligibility.feature' },
      loadScenario(zorgtoeslag.path, 'eligibility.feature'),
    );
  });

  test('closing the last tab lands on it without hanging', async ({ page }) => {
    const root = `/trajecten/${TEST_TRAJECT_REF}/editor`;
    await page.goto(`${root}/kieswet`);
    await expect(tabItems(page)).toHaveCount(1, { timeout: 20_000 });

    await closeButton(page).click({ force: true, timeout: 5_000 });

    expect(await answers(page), 'the page answers after the last tab closes').toBe(true);
    await expect(page).toHaveURL(new RegExp(`${root}$`));
    await expect(tabItems(page)).toHaveCount(0);
    await expect(neutralState(page)).toBeVisible();
  });

  test('closing the active tab while another stays open opens that one', async ({ page }) => {
    const root = `/trajecten/${TEST_TRAJECT_REF}/editor`;
    // Tabs persist across loads, so two loads leave two tabs, the second active.
    await page.goto(`${root}/kieswet`);
    await expect(tabItems(page)).toHaveCount(1, { timeout: 20_000 });
    await page.goto(`${root}/wet_op_de_zorgtoeslag`);
    await expect(tabItems(page)).toHaveCount(2, { timeout: 20_000 });
    await expect(activeTab(page)).toHaveAttribute('data-tab-key', /^wet_op_de_zorgtoeslag:/);

    await closeButton(page).click({ force: true, timeout: 5_000 });

    expect(await answers(page)).toBe(true);
    await expect(tabItems(page)).toHaveCount(1);
    await expect(activeTab(page)).toHaveAttribute('data-tab-key', /^kieswet:/);
    await expect(page).toHaveURL(new RegExp(`${root}/kieswet`));
  });

  test('going there with a tab open restores that tab', async ({ page }) => {
    const root = `/trajecten/${TEST_TRAJECT_REF}/editor`;
    await page.goto(`${root}/kieswet`);
    await expect(tabItems(page)).toHaveCount(1, { timeout: 20_000 });

    // The navigation to the root must go through: a blocked one leaves the
    // URL on kieswet and would pass the check below for the wrong reason.
    expect(await navigateInApp(page, root)).toBe(true);

    await expect(page).toHaveURL(new RegExp(`${root}/kieswet`), { timeout: 10_000 });
    await expect(tabItems(page)).toHaveCount(1);
  });
});
