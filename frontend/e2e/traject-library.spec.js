import { test, expect } from '@playwright/test';
import { mockAuthedEditor, TEST_TRAJECT_REF } from './helpers.js';

// The traject library (Home inside a traject): "In dit traject" with its quick
// search, "Recent bekeken" next to it, and "Toevoegen aan traject" on an open
// rulework that comes from a source the traject only reads along with.

const OWN = { source_id: 'traject-eigen', source_name: 'Eigen traject-repo', source_priority: 0 };
const CENTRAL = { source_id: 'central', source_name: 'Centraal corpus', source_priority: 2 };

/** `n` ruleworks in the traject's own repository: "Wet 001" … */
function ownLaws(n) {
  return Array.from({ length: n }, (_, i) => {
    const nr = String(i + 1).padStart(3, '0');
    return { law_id: `wet_${nr}`, display_name: `Wet ${nr}`, ...OWN };
  });
}
const CENTRAL_LAW = { law_id: 'centrale_wet', display_name: 'Centrale wet', ...CENTRAL };

function lawYaml(law) {
  return `$id: ${law.law_id}\nregulatory_layer: WET\npublication_date: '2025-01-01'\nvalid_from: '2025-01-01'\nname: ${law.display_name}\narticles:\n  - number: '1'\n    text: De tekst van artikel 1.\n`;
}

/**
 * Serve a traject whose own repository holds `own`, next to one rulework from
 * the central corpus. Returns the ids a promote call was made for.
 */
async function mockTrajectLibrary(page, own) {
  await mockAuthedEditor(page);
  const catalog = [...own, CENTRAL_LAW];
  const changed = own.map((law) => law.law_id);
  const promoted = [];
  const json = (body, headers = {}) => ({
    status: 200,
    contentType: 'application/json',
    headers,
    body: JSON.stringify(body),
  });
  const base = `/api/trajects/${TEST_TRAJECT_REF}`;
  await page.route(`**${base}/**`, (route, request) => {
    const url = new URL(request.url());
    const path = url.pathname.slice(base.length);
    if (path === '/corpus/changed-laws') return route.fulfill(json(changed));
    if (path === '/corpus/laws') {
      const ids = url.searchParams.get('ids');
      const laws = ids ? catalog.filter((law) => ids.split(',').includes(law.law_id)) : [];
      return route.fulfill(json(laws, { 'X-Total-Count': String(laws.length) }));
    }
    const promote = path.match(/^\/corpus\/laws\/([^/]+)\/promote$/);
    if (promote && request.method() === 'POST') {
      const law = catalog.find((l) => l.law_id === promote[1]);
      promoted.push(law.law_id);
      // From here on the rulework sits in the traject's own repository.
      Object.assign(law, OWN);
      changed.push(law.law_id);
      return route.fulfill(json({}));
    }
    const body = path.match(/^\/corpus\/laws\/([^/]+)$/);
    if (body) {
      const law = catalog.find((l) => l.law_id === body[1]);
      if (!law) return route.fulfill({ status: 404, body: '' });
      return route.fulfill({ status: 200, contentType: 'text/yaml', body: lawYaml(law) });
    }
    return route.fulfill(json([]));
  });
  return { promoted };
}

/** What "Recent bekeken" holds for the traject, most recent first. */
async function seedRecent(page, laws) {
  await page.addInitScript(
    ([key, value]) => localStorage.setItem(key, value),
    [
      `regelrecht-recent-laws:${TEST_TRAJECT_REF}`,
      JSON.stringify(laws.map((law) => ({ law_id: law.law_id, name: law.display_name }))),
    ],
  );
}

const sectionRows = (page, key) => page.locator(`nldd-list-item[data-section="${key}"]`);
const filterField = (page) => page.locator('nldd-search-field[data-testid="traject-laws-filter"]');
const addButton = (page) => page.locator('nldd-button[data-testid="add-open-law-to-traject"]');

test('the quick search finds a rulework folded away under "Toon alle"', async ({ page }) => {
  await mockTrajectLibrary(page, ownLaws(25));
  await page.goto(`/trajecten/${TEST_TRAJECT_REF}`);

  await expect(sectionRows(page, 'traject')).toHaveCount(20);
  await expect(page.getByRole('button', { name: 'Toon alle 25' })).toBeVisible();

  await filterField(page).locator('input').fill('wet 023');
  await expect(sectionRows(page, 'traject')).toHaveCount(1);
  await expect(sectionRows(page, 'traject').first()).toContainText('Wet 023');
  await expect(page.getByText('1 regelwerk', { exact: true })).toBeVisible();

  await filterField(page).locator('input').fill('bestaat niet');
  await expect(sectionRows(page, 'traject')).toHaveCount(0);
  await expect(page.getByText('Geen regelwerken gevonden')).toBeVisible();

  await filterField(page).locator('input').fill('');
  await expect(sectionRows(page, 'traject')).toHaveCount(20);
  await expect(page.getByRole('button', { name: 'Toon alle 25' })).toBeVisible();
});

test('a short list has no quick search', async ({ page }) => {
  await mockTrajectLibrary(page, ownLaws(5));
  await page.goto(`/trajecten/${TEST_TRAJECT_REF}`);

  await expect(sectionRows(page, 'traject')).toHaveCount(5);
  await expect(filterField(page)).toHaveCount(0);
});

test('"Recent bekeken" leaves out what "In dit traject" already lists', async ({ page }) => {
  const own = ownLaws(5);
  await mockTrajectLibrary(page, own);
  await seedRecent(page, [own[1], CENTRAL_LAW]);
  await page.goto(`/trajecten/${TEST_TRAJECT_REF}`);

  await expect(sectionRows(page, 'traject')).toHaveCount(5);
  await expect(sectionRows(page, 'recent')).toHaveCount(1);
  await expect(sectionRows(page, 'recent').first()).toContainText('Centrale wet');

  // Opening a rulework of the traject leaves the list as it was.
  await sectionRows(page, 'traject').nth(3).click();
  await expect(page).toHaveURL(/\/corpus\/wet_004/);
  await expect(sectionRows(page, 'recent')).toHaveCount(1);
  await expect(sectionRows(page, 'recent').first()).toContainText('Centrale wet');
});

test('an open rulework from the central corpus can be added to the traject', async ({ page }) => {
  const { promoted } = await mockTrajectLibrary(page, ownLaws(3));
  await page.goto(`/trajecten/${TEST_TRAJECT_REF}/corpus/${CENTRAL_LAW.law_id}`);

  await expect(addButton(page)).toBeVisible();
  await addButton(page).click();

  await expect(addButton(page)).toHaveCount(0);
  expect(promoted).toEqual([CENTRAL_LAW.law_id]);
  await expect(sectionRows(page, 'traject')).toHaveCount(4);
  await expect(sectionRows(page, 'traject').filter({ hasText: 'Centrale wet' })).toHaveCount(1);
});

test('a rulework of the traject itself has no "Toevoegen aan traject"', async ({ page }) => {
  await mockTrajectLibrary(page, ownLaws(3));
  await page.goto(`/trajecten/${TEST_TRAJECT_REF}/corpus/wet_001`);

  await expect(page.locator('nldd-toolbar[accessible-label="Regelwerkacties"]')).toBeVisible();
  await expect(addButton(page)).toHaveCount(0);
});
