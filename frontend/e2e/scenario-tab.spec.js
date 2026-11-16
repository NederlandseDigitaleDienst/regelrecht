/**
 * A scenario file in its own editor tab.
 *
 * - A deep link opens the file as a tab, next to the regelwerk's articles.
 * - Editing a scenario marks its tab as unsaved with a dot (custom CSS for a
 *   gap in the design system, src/styles/design-system-gaps.css). This spec is
 *   what notices when a design-system upgrade stops drawing it.
 * - The edits survive a switch to an article tab, and the read-only scenario
 *   pane there says it runs them.
 * - Closing the tab with unsaved edits asks first; saving writes the file.
 *
 * The corpus, the file and the save are mocked from disk (helpers-corpus.js).
 * Requires the WASM engine (`just wasm-build`).
 */
import { test, expect } from '@playwright/test';
import * as yaml from 'js-yaml';
import { loadFixture, mockAuthedEditor, clearOpenTabsStorage, readYamlSource, setYamlPane, TEST_TRAJECT_REF } from './helpers.js';
import { mockCorpusApi } from './helpers-corpus.js';

const FILE = 'typed.feature';
const SCENARIO = `Feature: Typed input controls

  Scenario: Mixed parameter types
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "is_verzekerde" is "true"
    Given parameter "toetsingsinkomen" is 150000
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
`;

const scenarioUrl = `/trajecten/${TEST_TRAJECT_REF}/editor/wet_op_de_zorgtoeslag/scenarios/${FILE}`;

/** The document tab bar item of the scenario file. */
const scenarioTab = (page) => page.locator(`nldd-document-tab-bar-item[text="${FILE}"]`);

/** Type into an nldd-text-field the way its own input event reports it. */
const typeInto = (field, value) =>
  field.evaluate((el, v) => {
    el.value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);

/** Navigate inside the app, as a link would: no reload, so memory survives. */
const navigate = (page, path) =>
  page.evaluate((to) => document.querySelector('#app').__vue_app__.config.globalProperties.$router.push(to), path);

/** Dismiss a tab the way the tab bar's close button does. */
const dismissTab = (tab) =>
  tab.evaluate((el) => {
    el.closest('nldd-document-tab-bar').dispatchEvent(
      new CustomEvent('tab-dismiss', { detail: { item: el, nextItem: null }, bubbles: true }),
    );
  });

/** Whether the unsaved dot is drawn on an element (its ::before). */
const dotDrawn = (locator) =>
  locator.evaluate((el) => {
    const style = getComputedStyle(el, '::before');
    return style.content !== 'none' && style.width === '7px' && style.position === 'absolute';
  });

test.describe('Scenario tab', () => {
  let puts;

  test.beforeEach(async ({ page }) => {
    await clearOpenTabsStorage(page);
    const fixture = loadFixture('zorgtoeslag-full.yaml');
    const corpus = new Map([
      ['wet_op_de_zorgtoeslag', { content: fixture, path: '', pubDate: '2024-12-20' }],
    ]);
    await mockCorpusApi(page, corpus, { id: 'wet_op_de_zorgtoeslag', scenarioFilename: FILE }, SCENARIO);
    // The save: record the body, answer like the editor-api does.
    puts = [];
    await page.route(`**/corpus/laws/wet_op_de_zorgtoeslag/scenarios/${FILE}`, (route, request) => {
      if (request.method() !== 'PUT') return route.fallback();
      puts.push(request.postData());
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        headers: { ETag: '"v2"' },
        body: JSON.stringify({ pr: null, etag: '"v2"' }),
      });
    });
    await mockAuthedEditor(page);
  });

  test('opens from a deep link, marks unsaved edits, keeps them across tabs, asks before closing, saves', async ({ page }) => {
    await page.goto(scenarioUrl);
    const screen = page.locator('[data-testid="scenario-screen"]');
    await expect(screen).toBeVisible({ timeout: 30_000 });
    await expect(scenarioTab(page)).toHaveCount(1);
    expect(await dotDrawn(scenarioTab(page))).toBe(false);

    // Edit a parameter: the tab gets the dot, the form offers to save.
    const form = page.locator('[data-testid="scenario-form-pane"]');
    const bsn = form.locator('nldd-list-item').filter({ hasText: 'bsn' }).locator('nldd-text-field');
    await bsn.waitFor({ timeout: 30_000 });
    await typeInto(bsn, '111222333');
    await expect(scenarioTab(page)).toHaveAttribute('data-unsaved', 'true');
    expect(await dotDrawn(scenarioTab(page))).toBe(true);
    await expect(page.locator('[data-testid="save-scenarios-btn"]')).toBeVisible();

    // To an article of the same regelwerk: no question, and the pane there
    // says it runs the tab's unsaved edits.
    await navigate(page, `/trajecten/${TEST_TRAJECT_REF}/editor/wet_op_de_zorgtoeslag/2`);
    await page.locator('nldd-document-tab-bar-item[text="Artikel 2"]').waitFor({ timeout: 30_000 });
    await expect(page.locator('[data-testid="leave-confirm"][open]')).toHaveCount(0);
    const pane = page.locator('[data-testid="scenario-status-pane"]');
    await expect(pane.locator(`nldd-banner[text="Niet-opgeslagen wijzigingen in het tabblad ${FILE}"]`)).toBeVisible({ timeout: 30_000 });

    // Back to the tab: the edit is still there.
    await scenarioTab(page).click();
    await expect(screen).toBeVisible();
    await expect(page.locator('[data-testid="save-scenarios-btn"]')).toBeVisible();

    // Saving writes the edit and clears the dot.
    await page.locator('[data-testid="save-scenarios-btn"]').click();
    await expect.poll(() => puts.length).toBe(1);
    expect(puts[0]).toContain('Given parameter "bsn" is 111222333');
    await expect(scenarioTab(page)).not.toHaveAttribute('data-unsaved', 'true');
    expect(await dotDrawn(scenarioTab(page))).toBe(false);
  });

  test('asks before closing a tab with unsaved edits, and keeps it on "Blijf hier"', async ({ page }) => {
    await page.goto(scenarioUrl);
    const form = page.locator('[data-testid="scenario-form-pane"]');
    const bsn = form.locator('nldd-list-item').filter({ hasText: 'bsn' }).locator('nldd-text-field');
    await bsn.waitFor({ timeout: 30_000 });
    await typeInto(bsn, '111222333');
    await expect(scenarioTab(page)).toHaveAttribute('data-unsaved', 'true');

    // The tab bar's dismiss: ask, stay.
    await dismissTab(scenarioTab(page));
    const dialog = page.locator('[data-testid="leave-confirm"]');
    await expect(dialog).toHaveAttribute('text', `Je hebt niet-opgeslagen wijzigingen in ${FILE}`);
    await page.locator('[data-testid="leave-stay-btn"]').click();
    await expect(scenarioTab(page)).toHaveCount(1);
    await expect(page.locator('[data-testid="save-scenarios-btn"]')).toBeVisible();
  });

  test('keeps the article\'s edits behind its scenario tab, and asks before closing the article\'s tab', async ({ page }) => {
    await page.goto(`/trajecten/${TEST_TRAJECT_REF}/editor/wet_op_de_zorgtoeslag/2`);
    const articleTab = page.locator('nldd-document-tab-bar-item[text="Artikel 2"]');
    await articleTab.waitFor({ timeout: 30_000 });
    const mr = yaml.load(await readYamlSource(page));
    mr.definitions = { ...(mr.definitions ?? {}), e2e_bewerking: { value: 1 } };
    await setYamlPane(page, yaml.dump(mr, { lineWidth: 80, noRefs: true }));
    await expect(articleTab).toHaveAttribute('data-unsaved', 'true');

    // To the scenario file of the same rulework: no question, and the
    // article keeps its edits (and its dot) behind it.
    await navigate(page, scenarioUrl);
    await expect(page.locator('[data-testid="scenario-screen"]')).toBeVisible({ timeout: 30_000 });
    await expect(page.locator('[data-testid="leave-confirm"][open]')).toHaveCount(0);
    await expect(articleTab).toHaveAttribute('data-unsaved', 'true');

    // Closing the article's tab from here drops its edits: ask, and stay.
    await dismissTab(articleTab);
    const dialog = page.locator('[data-testid="leave-confirm"]');
    await expect(dialog).toHaveAttribute('text', 'Je hebt niet-opgeslagen wijzigingen in artikel 2');
    await page.locator('[data-testid="leave-stay-btn"]').click();
    await expect(articleTab).toHaveCount(1);
    await expect(articleTab).toHaveAttribute('data-unsaved', 'true');
  });
});
