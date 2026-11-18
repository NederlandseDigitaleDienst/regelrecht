/**
 * Leaving an article with unsaved edits asks first.
 *
 * The editor keeps only the selected article's edits, so switching away drops
 * them. Before that happens it asks: "Blijf hier" keeps the edits, the article
 * and the tab bar's selection; "Wijzigingen weggooien" carries on. The tab bar
 * marks a clicked tab as selected before the editor answers, so staying has to
 * put that back - which only a browser shows, hence this spec.
 *
 * Requires the WASM engine to be built (frontend/public/wasm/pkg).
 */
import { test, expect } from '@playwright/test';
import * as yaml from 'js-yaml';
import { gotoEditor, selectArticle, readYamlSource, setYamlPane, loadFixture } from './helpers.js';

const LAW = 'wet_op_de_zorgtoeslag';

const tab = (page, nr) => page.locator(`nldd-document-tab-bar-item[text="Artikel ${nr}"]`).first();
const stay = (page) => page.getByTestId('leave-stay-btn');
const discard = (page) => page.getByTestId('leave-discard-btn');

// Open articles 2 and 3 as tabs and come back to 2.
async function openTwoTabs(page) {
  const body = loadFixture('zorgtoeslag-full.yaml');
  await page.route(`**/corpus/laws/${LAW}`, (route) =>
    route.fulfill({ status: 200, contentType: 'text/yaml', body }),
  );
  await gotoEditor(page, LAW, '2');
  await selectArticle(page, '3');
  await selectArticle(page, '2');
  await expect(page).toHaveURL(/\/editor\/wet_op_de_zorgtoeslag\/2$/);
}

// A structural edit in the YAML pane: a definition the article did not have.
async function editArticle(page) {
  const mr = yaml.load(await readYamlSource(page));
  mr.definitions = { ...mr.definitions, e2e_unsaved: 1 };
  await setYamlPane(page, yaml.dump(mr));
}

test.describe('Leaving an article with unsaved edits', () => {
  test('staying keeps the edits, the article and the selected tab', async ({ page }) => {
    await openTwoTabs(page);
    await editArticle(page);

    await tab(page, '3').click();
    await expect(stay(page)).toBeVisible();
    await stay(page).click();

    await expect(stay(page)).toBeHidden();
    await expect(page).toHaveURL(/\/editor\/wet_op_de_zorgtoeslag\/2$/);
    expect(await readYamlSource(page)).toContain('e2e_unsaved');
    await expect(tab(page, '2')).toHaveJSProperty('selected', true);
    await expect(tab(page, '3')).toHaveJSProperty('selected', false);
  });

  test('discarding moves on and leaves nothing behind', async ({ page }) => {
    await openTwoTabs(page);
    await editArticle(page);

    await tab(page, '3').click();
    await discard(page).click();

    await expect(page).toHaveURL(/\/editor\/wet_op_de_zorgtoeslag\/3$/);
    await expect(tab(page, '3')).toHaveJSProperty('selected', true);

    // Back on article 2 the edit is gone, and nothing is asked on the way.
    await tab(page, '2').click();
    await expect(page).toHaveURL(/\/editor\/wet_op_de_zorgtoeslag\/2$/);
    await expect(stay(page)).toBeHidden();
    expect(await readYamlSource(page)).not.toContain('e2e_unsaved');
  });

  test('closing the edited tab asks too, and staying keeps it open', async ({ page }) => {
    await openTwoTabs(page);
    await editArticle(page);

    await tab(page, '2').hover();
    await tab(page, '2').getByRole('button').click();
    await expect(stay(page)).toBeVisible();
    await stay(page).click();

    await expect(tab(page, '2')).toBeVisible();
    await expect(tab(page, '2')).toHaveJSProperty('selected', true);
    await expect(tab(page, '3')).toHaveJSProperty('selected', false);
    expect(await readYamlSource(page)).toContain('e2e_unsaved');
  });

  // The text editor renumbers ordered lists when it takes in a text, and the
  // paragraphs of article 2 ("1.", "2.") each read as a list of one. Coming
  // back to it must not count as an edit, or leaving would ask for nothing.
  test('going back to an article with numbered paragraphs leaves it unedited', async ({ page }) => {
    await openTwoTabs(page);
    await expect(page).not.toHaveTitle(/^●/);

    await tab(page, '3').click();
    await expect(page).toHaveURL(/\/editor\/wet_op_de_zorgtoeslag\/3$/);
    await expect(stay(page)).toBeHidden();
  });
});
