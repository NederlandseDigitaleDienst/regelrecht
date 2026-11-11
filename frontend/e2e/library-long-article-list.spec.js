import { test, expect } from '@playwright/test';
import { mockAuthedEditor } from './helpers.js';

// A regelwerk with thousands of articles renders only the rows in view (see
// useVirtualRows). The largest ones in the corpus have over six thousand, and
// rendering every row froze the library for minutes. Generated here rather
// than taken from the corpus: the test needs the size, not the content.

const LAW_ID = 'wet_met_veel_artikelen';
const ARTICLES = 1500;

function longLaw() {
  const articles = Array.from({ length: ARTICLES }, (_, i) => {
    const n = i + 1;
    return `  - number: '${n}'\n    text: Artikel ${n} van een lange wet, met een eerste regel die als omschrijving dient.\n`;
  }).join('');
  return `$id: ${LAW_ID}\nregulatory_layer: WET\npublication_date: '2025-01-01'\nvalid_from: '2025-01-01'\nname: Wet met veel artikelen\narticles:\n${articles}`;
}

async function openLongLaw(page) {
  await mockAuthedEditor(page);
  const json = (body) => ({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
  await page.route('**/api/corpus/laws?**', (route) =>
    route.fulfill(json([{ law_id: LAW_ID, name: 'Wet met veel artikelen', source_id: 'central', source_name: 'Centraal', source_priority: 2, enriched: false }])),
  );
  await page.route(`**/api/corpus/laws/${LAW_ID}`, (route) =>
    route.fulfill({ status: 200, contentType: 'text/yaml', body: longLaw() }),
  );
  await page.route(`**/api/corpus/laws/${LAW_ID}/**`, (route) => route.fulfill(json([])));
  await page.goto(`/corpus-juris/${LAW_ID}`);
  await expect(articleRow(page, '1')).toBeVisible();
}

const rows = (page) => page.locator('nldd-list-item[data-row-key]');
const articleRow = (page, number) => page.locator(`nldd-list-item[data-row-key="${number}"]`);

/**
 * Scroll the law pane's page to `fraction` of its height, and wait until the
 * rendered rows have followed (they move one animation frame later).
 */
async function scrollPane(page, fraction) {
  const before = await rows(page).first().getAttribute('data-row-key');
  await page.evaluate((f) => {
    const scroller = document.querySelector('nldd-list-item[data-row-key]').closest('nldd-page').scrollTarget;
    scroller.scrollTop = f * (scroller.scrollHeight - scroller.clientHeight);
  }, fraction);
  await expect(rows(page).first()).not.toHaveAttribute('data-row-key', before);
}

test('a long article list renders a window, not every row', async ({ page }) => {
  await openLongLaw(page);
  const rendered = await rows(page).count();
  expect(rendered).toBeGreaterThan(5);
  expect(rendered).toBeLessThan(150);

  // Every row says where it sits in the whole list.
  await expect(articleRow(page, '1')).toHaveAttribute('aria-setsize', String(ARTICLES));
  await expect(articleRow(page, '1')).toHaveAttribute('aria-posinset', '1');
});

test('scrolling to the end shows the last article, and back the first', async ({ page }) => {
  await openLongLaw(page);
  await scrollPane(page, 1);
  await expect(articleRow(page, String(ARTICLES))).toBeVisible();
  await expect(articleRow(page, String(ARTICLES))).toHaveAttribute('aria-posinset', String(ARTICLES));
  await expect(articleRow(page, '1')).toHaveCount(0);

  await scrollPane(page, 0);
  await expect(articleRow(page, '1')).toBeVisible();
});

test('the arrow keys go on past the rows that were rendered at first', async ({ page }) => {
  await openLongLaw(page);
  const firstWindow = await rows(page).count();
  await articleRow(page, '1').click();
  // In steps, so the window (which follows one animation frame later) keeps
  // up the way it does for a person holding the key.
  for (let i = 0; i < firstWindow + 20; i++) {
    await page.keyboard.press('ArrowDown');
    if (i % 10 === 9) await page.waitForTimeout(50);
  }
  // Focus sits on a control inside the row; the row is the nearest
  // nldd-list-item around it, through the shadow roots.
  const focused = await page.evaluate(() => {
    let el = document.activeElement;
    while (el?.shadowRoot?.activeElement) el = el.shadowRoot.activeElement;
    while (el && el.tagName !== 'NLDD-LIST-ITEM') el = el.parentElement ?? el.getRootNode().host;
    return Number(el?.getAttribute('aria-posinset'));
  });
  expect(focused).toBeGreaterThan(firstWindow);
});

test('clicking a row far down selects that article', async ({ page }) => {
  await openLongLaw(page);
  await scrollPane(page, 0.5);
  // A row in view, not one in the margin above it.
  const number = await page.evaluate(() => {
    for (const row of document.querySelectorAll('nldd-list-item[data-row-key]')) {
      if (row.getBoundingClientRect().top > 200) return row.getAttribute('data-row-key');
    }
    return null;
  });
  const row = articleRow(page, number);
  await row.click();
  await expect(page).toHaveURL(new RegExp(`/corpus-juris/${LAW_ID}/${number}$`));
  await expect(articleRow(page, number)).toHaveAttribute('current', '');
});

test('a filtered list under the limit renders every row', async ({ page }) => {
  await openLongLaw(page);
  await page.getByRole('button', { name: 'Artikelen zoeken' }).click();
  await page.getByRole('searchbox', { name: 'Artikelen zoeken' }).fill('Artikel 14');
  // "Artikel 14", "Artikel 140".."149", "Artikel 1400".."1499": 111 rows.
  await expect(rows(page)).toHaveCount(111);
  await expect(articleRow(page, '1')).toHaveCount(0);
  await expect(articleRow(page, '14')).not.toHaveAttribute('aria-setsize');
});

test('keyboard focus stays in the list when its row scrolls away', async ({ page }) => {
  await openLongLaw(page);
  await articleRow(page, '1').click();
  await scrollPane(page, 0.7);
  await expect(articleRow(page, '1')).toHaveCount(0);
  const inList = await page.evaluate(() => {
    let el = document.activeElement;
    while (el?.shadowRoot?.activeElement) el = el.shadowRoot.activeElement;
    while (el && el.tagName !== 'NLDD-LIST-ITEM') el = el.parentElement ?? el.getRootNode().host;
    return el?.hasAttribute('data-row-key') ?? false;
  });
  expect(inList).toBe(true);
});

test.describe('on a phone, where the document scrolls', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test('scrolling down the page moves the window along', async ({ page }) => {
    await openLongLaw(page);
    // One column: the article list is the page, and the document scrolls.
    await page.evaluate(() => {
      const scroller = document.querySelector('nldd-list-item[data-row-key]').closest('nldd-page').scrollTarget;
      if (scroller !== document.scrollingElement) throw new Error('expected the document to scroll on a phone');
      scroller.scrollTop = 0.6 * (scroller.scrollHeight - scroller.clientHeight);
    });
    await expect(articleRow(page, '1')).toHaveCount(0);
    const inView = await page.evaluate(() =>
      [...document.querySelectorAll('nldd-list-item[data-row-key]')].some((row) => {
        const r = row.getBoundingClientRect();
        return r.bottom > 0 && r.top < innerHeight;
      }),
    );
    expect(inView).toBe(true);
  });
});
