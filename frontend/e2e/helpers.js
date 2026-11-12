import { readFileSync } from 'fs';
import { resolve } from 'path';
import * as yaml from 'js-yaml';

const FIXTURE_DIR = resolve(import.meta.dirname, 'fixtures');

/**
 * A placeholder traject ref used across the e2e mocks. It satisfies the
 * router's `{slug}-{8hex}` shape (`[a-z0-9-]+-[0-9a-f]{8}`), so the
 * authenticated, traject-scoped editor route accepts it and the law API
 * hangs under `/api/trajects/${TEST_TRAJECT_REF}/corpus/...`.
 */
export const TEST_TRAJECT_REF = 'test-traject-0a1b2c3d';
// A SECOND traject, so specs can exercise a traject switch (per-traject tabs).
export const TEST_TRAJECT_B_REF = 'test-traject-b-0b1c2d3e';

// Fake, non-personal signed-in identity for the mocked `/auth/status`. No real
// names, emails, roles or tokens — just the shape `useAuth` reads. `.test` is a
// reserved TLD, so the address can never resolve to a real mailbox.
const TEST_PERSON = {
  sub: 'e2e-test-user',
  name: 'E2E Test',
  email: 'e2e@example.test',
  roles: [],
};

// Minimal traject the mocked `/api/trajects` list/detail returns. `ref` MUST
// match `TEST_TRAJECT_REF` so the trajecten store resolves the active traject and
// `trajectMissing` stays false; the id is any UUID whose last 8 hex chars match
// the ref's suffix (the backend resolver keys off those).
const TEST_TRAJECT = {
  id: '00000000-0000-0000-0000-00000a1b2c3d',
  ref: TEST_TRAJECT_REF,
  name: 'E2E Test Traject',
  description: '',
  scope: '',
  status: 'bezig',
  role: 'owner',
};

const TEST_TRAJECT_B = {
  id: '00000000-0000-0000-0000-00000b1c2d3e',
  ref: TEST_TRAJECT_B_REF,
  name: 'E2E Test Traject B',
  description: '',
  scope: '',
  status: 'bezig',
  role: 'owner',
};

// The trajects the membership list returns. Both are always present so a spec
// can switch between them; single-traject specs simply ignore the second.
const TEST_TRAJECTS = [TEST_TRAJECT, TEST_TRAJECT_B];

/**
 * Load a YAML fixture file as a string.
 */
export function loadFixture(name) {
  return readFileSync(resolve(FIXTURE_DIR, name), 'utf-8');
}

/**
 * Wipe the editor's per-traject open-tab storage before a run so tabs don't
 * bleed across tests. The keys are now per-traject (`regelrecht-open-tabs:<ref>`
 * / `regelrecht-active-tab:<ref>`), so a single un-suffixed removeItem is a
 * no-op - clear by PREFIX instead. This matters more than before: a leftover
 * `regelrecht-active-tab:<ref>` now makes the editor auto-open a document and
 * replace the URL on entry.
 * @param {import('@playwright/test').Page} page
 */
export async function clearOpenTabsStorage(page) {
  await page.addInitScript(() => {
    try {
      for (const key of Object.keys(window.localStorage)) {
        if (key.startsWith('regelrecht-open-tabs') || key.startsWith('regelrecht-active-tab')) {
          window.localStorage.removeItem(key);
        }
      }
    } catch { /* ignore */ }
  });
}

/**
 * Mock the auth + traject endpoints so the `requiresAuth`, traject-scoped
 * editor routes mount under Playwright without a real backend or SSO. The
 * router guard awaits `/auth/status` (served as authenticated here), and
 * the trajecten store needs the active ref present in the `/api/trajects` list.
 *
 * The traject detail handler only answers the bare `/api/trajects/{id}`
 * endpoint and falls through for its `/corpus/...` sub-routes, leaving those
 * to the corpus mocks.
 *
 * @param {import('@playwright/test').Page} page
 */
export async function mockAuthedEditor(page) {
  await page.route('**/auth/status', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        authenticated: true,
        oidc_configured: true,
        person: TEST_PERSON,
      }),
    }),
  );
  // GET /api/trajects - the membership list the trajecten store reads.
  await page.route('**/api/trajects', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(TEST_TRAJECTS),
    }),
  );
  // GET /api/trajects/{ref} - single traject detail. Only the bare detail path;
  // `/api/trajects/{ref}/corpus/...` falls through to the corpus handlers.
  // Resolves whichever of the known trajects the URL segment names (falling
  // back to the first) so a two-traject spec gets the right detail per ref.
  await page.route('**/api/trajects/*', (route, request) => {
    const { pathname } = new URL(request.url());
    const match = pathname.match(/^\/api\/trajects\/([^/]+)$/);
    if (!match) return route.fallback();
    const ref = decodeURIComponent(match[1]);
    const traject = TEST_TRAJECTS.find((t) => t.ref === ref) ?? TEST_TRAJECT;
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        ...traject,
        members: [],
        pending_invites: [],
        sources: [],
      }),
    });
  });
}

/**
 * Intercept the law API and serve a local YAML fixture instead. The glob
 * matches both the global (`/api/corpus/laws/{lawId}`) and the traject-scoped
 * (`/api/trajects/{ref}/corpus/laws/{lawId}`) endpoint, so the authenticated
 * editor - which reads through the active traject - is served the same fixture
 * as any legacy global caller.
 * @param {import('@playwright/test').Page} page
 * @param {string} lawId - e.g. 'wet_op_de_zorgtoeslag'
 * @param {string} fixtureName - e.g. 'zorgtoeslag-stripped.yaml'
 */
export async function interceptLaw(page, lawId, fixtureName) {
  const body = loadFixture(fixtureName);
  await page.route(`**/corpus/laws/${lawId}`, (route) =>
    route.fulfill({
      status: 200,
      contentType: 'text/yaml',
      body,
    }),
  );
  // Also intercept the default law id from the fixture itself
  if (lawId !== 'wet_op_de_zorgtoeslag') {
    await page.route('**/corpus/laws/wet_op_de_zorgtoeslag', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'text/yaml',
        body,
      }),
    );
  }
}

/**
 * Navigate to the traject-scoped editor and wait for it to load. Sets up the
 * auth + traject mocks first so the `requiresAuth` route mounts, then goes to
 * `/trajecten/${TEST_TRAJECT_REF}/editor/${lawId}[/${articleNumber}]`.
 * @param {import('@playwright/test').Page} page
 * @param {string} [lawId] - law id path segment
 * @param {string|number} [articleNumber] - optional article path segment
 */
export async function gotoEditor(page, lawId = 'wet_op_de_zorgtoeslag', articleNumber) {
  await mockAuthedEditor(page);
  const article = articleNumber != null ? `/${articleNumber}` : '';
  await page.goto(`/trajecten/${TEST_TRAJECT_REF}/editor/${lawId}${article}`);
  // Wait for the document tab bar to appear (articles loaded)
  await page.waitForSelector('nldd-document-tab-bar-item', { timeout: 10_000 });
}

/**
 * Make an article the active editing target.
 *
 * The editor's document tab bar only lists already-open (law, article) tabs,
 * and each item carries its label in the web component's `text` attribute
 * (shadow DOM), not as light-DOM text. So we match on that attribute. When the
 * requested article has no tab yet, we open it by pointing the traject-scoped
 * editor route at that article - exactly the state opening it from the document
 * list produces - then wait for and select its tab.
 * @param {import('@playwright/test').Page} page
 * @param {string|number} articleNumber
 */
export async function selectArticle(page, articleNumber) {
  const target = String(articleNumber);
  const tab = () =>
    page.locator(`nldd-document-tab-bar-item[text="Artikel ${target}"]`).first();
  if ((await tab().count()) === 0) {
    const { pathname, search } = new URL(page.url());
    // .../editor/{lawId}[/{article}] -> swap in (or append) the article segment.
    const next = pathname.replace(/(\/editor\/[^/]+)(?:\/[^/]+)?$/, `$1/${target}`);
    await page.goto(next + search);
    await tab().waitFor({ timeout: 10_000 });
  }
  await tab().click();
  // Small wait for reactivity to settle
  await page.waitForTimeout(200);
}

/**
 * The YAML pane's code editor. The editor moved from a plain
 * `.editor-yaml-textarea` to the design-system `nldd-code-editor` web
 * component, bound to the selected article's machine_readable YAML.
 * @param {import('@playwright/test').Page} page
 */
export function yamlEditor(page) {
  return page.locator('nldd-code-editor').first();
}

/**
 * Raw string content of the YAML code editor (the selected article's
 * machine_readable, serialised). Specs that string-manipulate the YAML read
 * this, edit it, and hand it back through {@link setYamlPane}.
 * @param {import('@playwright/test').Page} page
 * @returns {Promise<string>}
 */
export async function readYamlSource(page) {
  return yamlEditor(page).evaluate((el) => el.value ?? '');
}

/**
 * Replace the YAML code editor content, driving the same input path a user's
 * keystrokes take. `nldd-code-editor` reads the new value from
 * `event.detail.value`, so we dispatch that shape directly - robust against
 * the component's internal editor state.
 * @param {import('@playwright/test').Page} page
 * @param {string} text
 */
export async function setYamlPane(page, text) {
  await yamlEditor(page).evaluate((el, val) => {
    el.value = val;
    el.dispatchEvent(new CustomEvent('input', { detail: { value: val }, bubbles: true }));
  }, text);
}

/**
 * Read the YAML pane content and parse it.
 * @param {import('@playwright/test').Page} page
 * @returns {Promise<object|null>}
 */
export async function readYamlPane(page) {
  const text = await readYamlSource(page);
  if (!text.trim()) return null;
  return yaml.load(text);
}

/**
 * Get one of the editor's panes by its view id ('text', 'machine', 'scenario',
 * 'yaml', 'notes').
 *
 * Without a stored preference the editor opens every pane at once, which is
 * what a fresh CI browser gets. Components that appear in more than one pane -
 * MachineEmptyState sits in both Machine and YAML - therefore put their
 * test ids in the DOM twice. Scope through this helper rather than reaching for
 * `.first()`: the ambiguity is real, and the test should say which pane it
 * means.
 * @param {import('@playwright/test').Page} page
 * @param {string} view
 */
export function pane(page, view) {
  return page.locator(`[data-testid="pane-${view}"]`);
}

/**
 * Get the machine_readable pane element.
 * @param {import('@playwright/test').Page} page
 */
export function machineReadablePane(page) {
  return page.locator('[data-testid="machine-readable"]');
}

/**
 * Open the ActionSheet in edit mode for the machine-readable action whose
 * output name is `output`. The row's edit affordance moved from an inline
 * "Bewerk" button to a RowActionsMenu ("more" icon) whose Edit menu-item
 * carries the `action-<output>-edit-btn` test-id. The menu-item lives in a
 * closed popover, so a direct DOM click (evaluate) fires its handler without
 * needing the popover open - the same pattern the sheet-save helpers use.
 * @param {import('@playwright/test').Page} page
 * @param {string} output
 */
export async function openActionEditor(page, output) {
  await page
    .locator(`[data-testid="action-${output}-edit-btn"]`)
    .first()
    .evaluate((el) => el.click());
  await waitForEditSheet(page);
}

/**
 * Remove the i-th value row of the currently selected operation in the open
 * ActionSheet. The inline "minus" icon-button was replaced by a per-row
 * actions menu ("more" icon) whose "Verwijder" item drives removeValue. The
 * item sits in a closed popover, so we click it directly via the DOM.
 * @param {import('@playwright/test').Page} page
 * @param {number} index
 */
export async function removeOpValue(page, index) {
  await openSheet(page)
    .locator(`[data-testid="op-value-${index}"] nldd-menu-item[text="Verwijder"]`)
    .first()
    .evaluate((el) => el.click());
  await page.waitForTimeout(200);
}

/**
 * The single open edit/action sheet. The editor keeps several `nldd-sheet`
 * hosts mounted at once (one per dialog kind), all but the open one hidden, so
 * a bare `nldd-sheet` locator is ambiguous. Filtering on visibility targets the
 * one dialog that is actually open.
 * @param {import('@playwright/test').Page} page
 */
export function openSheet(page) {
  return page.locator('nldd-sheet:visible');
}

/**
 * Wait for the edit sheet to be visible.
 * @param {import('@playwright/test').Page} page
 */
export async function waitForEditSheet(page) {
  await openSheet(page).first().waitFor({ state: 'visible', timeout: 5000 });
  await page.waitForTimeout(100);
}

/**
 * Click "Opslaan" in the ActionSheet. The save button carries a stable
 * `action-sheet-save-btn` test-id and only renders while editable and
 * new/dirty.
 * @param {import('@playwright/test').Page} page
 */
export async function saveActionSheet(page) {
  await openSheet(page)
    .locator('[data-testid="action-sheet-save-btn"]')
    .evaluate((el) => el.click());
  await page.waitForTimeout(200);
}

/**
 * Wait for an nldd-sheet dialog to be open (Lit component uses internal <dialog>).
 * @param {import('@playwright/test').Page} page
 */
export async function waitForSheet(page) {
  await page.waitForFunction(() => {
    return [...document.querySelectorAll('nldd-sheet')].some(
      (sheet) => sheet.shadowRoot?.querySelector('dialog')?.open ?? false,
    );
  }, { timeout: 5000 });
  await page.waitForTimeout(200);
}

/**
 * Fill an nldd-text-field input inside nldd-sheet by label text.
 * Uses evaluate to bypass shadow DOM visibility issues.
 */
export async function fillSheetTextField(page, labelText, value) {
  const sheet = openSheet(page);
  const listItem = sheet.locator(`nldd-list-item:has(nldd-text-cell:has-text("${labelText}"))`);
  const input = listItem.locator('nldd-text-field input');
  await input.evaluate((el, val) => {
    el.value = val;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

/**
 * Fill an nldd-number-field input inside nldd-sheet by label text.
 * Dispatches both native and custom events for Vue binding.
 */
export async function fillSheetNumberField(page, labelText, value) {
  const sheet = openSheet(page);
  const listItem = sheet.locator(`nldd-list-item:has(nldd-text-cell:has-text("${labelText}"))`);
  const numberField = listItem.locator('nldd-number-field');
  await numberField.evaluate((el, val) => {
    const input = el.shadowRoot?.querySelector('input') ?? el.querySelector('input');
    if (input) {
      input.value = String(val);
      input.dispatchEvent(new Event('input', { bubbles: true }));
      input.dispatchEvent(new Event('change', { bubbles: true }));
    }
    el.dispatchEvent(new CustomEvent('change', { detail: { value: Number(val) }, bubbles: true }));
  }, value);
}

/**
 * Set an nldd-combo-box value inside the open sheet by its test-id. The
 * combo-box's change handler reads `event.detail.value`, so we dispatch that
 * shape directly - it does not require a matching menu-item to be present,
 * which lets a spec bind a source regulation without mocking the full law
 * list.
 * @param {import('@playwright/test').Page} page
 * @param {string} testid
 * @param {string} value
 */
export async function setSheetComboBox(page, testid, value) {
  await openSheet(page)
    .locator(`[data-testid="${testid}"]`)
    .evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new CustomEvent('change', { detail: { value: val }, bubbles: true }));
    }, value);
}

/**
 * Select a value in an nldd-dropdown inside nldd-sheet by label text.
 */
export async function selectSheetDropdown(page, labelText, value) {
  const sheet = openSheet(page);
  const listItem = sheet.locator(`nldd-list-item:has(nldd-text-cell:has-text("${labelText}"))`);
  const select = listItem.locator('select');
  await select.evaluate((el, val) => {
    el.value = val;
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }, value);
}

/**
 * Poll a scenario card's result sheet until it reports the wanted status
 * ("Mislukt" / "Geslaagd"). Scenarios auto-execute sequentially once their
 * dependency laws finish loading, and the overview cards carry no pass/fail
 * badge, so we (re)open the result sheet and read its outcome, retrying until
 * execution has produced one. The result sheet reads fresh data from the
 * scenario form on every open, so re-clicking picks up a later run. Throws with
 * the last-seen sheet text if the status never appears within `timeout`.
 * @param {import('@playwright/test').Page} page
 * @param {string} title - substring of the scenario card's name
 * @param {string} want - 'Mislukt' or 'Geslaagd'
 * @param {number} [timeout]
 */
export async function expectScenarioResult(page, title, want, timeout = 60_000) {
  const card = page.locator('nldd-card').filter({ hasText: title });
  await card.waitFor({ state: 'visible', timeout: 30_000 });
  const deadline = Date.now() + timeout;
  let lastText = '';
  while (Date.now() < deadline) {
    await card.getByRole('button', { name: 'Resultaat' }).click();
    const sheet = openSheet(page);
    await sheet.waitFor({ state: 'visible', timeout: 10_000 });
    await page.waitForTimeout(500);
    if ((await sheet.getByText(want, { exact: false }).count()) > 0) {
      return;
    }
    lastText = (await sheet.innerText().catch(() => '')) || lastText;
    // Close and retry - a later auto-execute pass may have produced a result.
    await page.keyboard.press('Escape');
    await page.waitForTimeout(1000);
  }
  throw new Error(`scenario "${title}" never reported "${want}". Last sheet text:\n${lastText}`);
}

/**
 * Click "Opslaan" in the EditSheet (nldd-sheet). The footer save button
 * carries a stable `edit-sheet-save-btn` test-id and only renders once the
 * form is dirty.
 */
export async function saveSheet(page) {
  const btn = openSheet(page).locator('[data-testid="edit-sheet-save-btn"]');
  await btn.evaluate(el => el.click());
  await page.waitForTimeout(300);
}
