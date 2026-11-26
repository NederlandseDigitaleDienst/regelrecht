import { test, expect, request as apiRequest } from '@playwright/test';
// Reuse the selector helpers the mocked `e2e/` suite already owns instead of
// copying them (DRY). `machineReadablePane` returns the editor's
// machine-readable region; the smoke flow asserts against the exact same
// contract the mocked specs do.
import { machineReadablePane } from '../e2e/helpers.js';
import { STORAGE_STATE } from './global-setup.js';
import { createTraject, deleteTraject, readTrajectLaw } from './smoke-helpers.js';

// The canonical fixture law + a stable article. Present in the deployed corpus;
// a fresh traject forks it without a machine_readable block on article 1, which
// is exactly the "empty state -> author -> persist" path the smoke exercises.
const LAW_ID = 'wet_op_de_zorgtoeslag';
const ARTICLE = '1';

// Trajects created during the run, torn down in afterAll so a second run starts
// clean. Stored by UUID (`id`), which the owner-only DELETE takes.
const createdTrajects = [];
// The traject the editor steps run against (set by the create test).
let traject = null;

test.describe.configure({ mode: 'serial' });

test.afterAll(async () => {
  if (createdTrajects.length === 0) return;
  // A fresh request context carrying the same authenticated session as the
  // specs (afterAll runs outside a test's fixtures).
  const ctx = await apiRequest.newContext({
    baseURL: process.env.SMOKE_BASE_URL,
    storageState: STORAGE_STATE,
  });
  try {
    for (const id of createdTrajects) {
      await deleteTraject(ctx, id);
    }
  } finally {
    await ctx.dispose();
  }
});

test('1) /library toont de wettenlijst tegen de echte corpus', async ({ page }) => {
  await page.goto('/library');
  // `/library` redirects to Home; the SPA booting against the real backend
  // sets the document title.
  await expect(page).toHaveTitle(/RegelRecht/);

  // The corpus law list is served by the real editor-api. Assert the known
  // fixture law is in it - this is the data behind the wettenlijst.
  const res = await page.request.get('/api/corpus/laws?limit=1000');
  expect(res.ok()).toBeTruthy();
  const laws = await res.json();
  expect(Array.isArray(laws)).toBeTruthy();
  expect(laws.length).toBeGreaterThan(0);
  expect(laws.some((l) => l.law_id === LAW_ID)).toBeTruthy();

  // Open the law in the library so it renders in the UI, confirming the
  // wettenlijst is navigable, not just an API payload: the sidebar shows the
  // law entry and the title reflects the opened law.
  await page.goto(`/corpus-juris/${LAW_ID}`);
  await expect(page.locator(`nldd-list-item[data-law-id="${LAW_ID}"]`).first()).toBeVisible();
});

// The deployed image serves its own static files, and the compressed variants
// are written by the build (frontend/scripts/precompress.mjs), not by the
// server. If that step ever silently drops out, ServeDir falls back to the
// plain file and nothing breaks — the editor just gets ~4x heavier without a
// single error. This is the only place that notices.
test('1b) de gedeployede bundel wordt gecomprimeerd uitgeserveerd', async ({ page }) => {
  await page.goto('/');
  const bundle = await page.evaluate(
    () => document.querySelector('script[type="module"][src^="/assets/"]')?.src,
  );
  expect(bundle, 'geen hoofdbundel in index.html gevonden').toBeTruthy();

  const res = await page.request.get(bundle, {
    headers: { 'accept-encoding': 'gzip, deflate, br' },
  });
  expect(res.ok()).toBeTruthy();
  const headers = res.headers();
  expect(headers['content-encoding']).toMatch(/^(br|gzip)$/);
  // Without Vary a shared cache could hand the compressed body to a client
  // that cannot decode it.
  expect(headers['vary']).toContain('accept-encoding');
});

test('2) /trajecten rendert', async ({ page }) => {
  await page.goto('/trajecten');
  await expect(page.locator('#kies-traject-titel')).toBeVisible();
});

test('3) maakt een traject aan via de API', async ({ page }) => {
  traject = await createTraject(page.request);
  createdTrajects.push(traject.id);
  expect(traject.ref).toMatch(/^[a-z0-9-]+-[0-9a-f]{8}$/);
  expect(traject.role).toBe('owner');
});

test('4) editor opent de wet, kiest artikel en toont het machine-paneel', async ({ page }) => {
  expect(traject, 'traject uit stap 3').not.toBeNull();

  await page.goto(`/trajecten/${traject.ref}/editor/${LAW_ID}/${ARTICLE}`);
  // Article tabs appear once the traject-scoped law has loaded through the full
  // stack (editor-api + corpus + WASM).
  await expect(
    page.locator('nldd-document-tab-bar-item', { hasText: `Artikel ${ARTICLE}` }).first(),
  ).toBeVisible({ timeout: 60_000 });

  // The machine-readable pane must resolve to one of its two KNOWN states -
  // populated content or the "no machine-readable yet" empty state - not stay
  // stuck loading. On a fresh traject fork it is the empty state.
  const emptyState = page.locator('[data-testid="no-machine-readable"]');
  await expect(emptyState.or(machineReadablePane(page)).first()).toBeVisible({ timeout: 30_000 });
});

test('5) lichte edit -> opslaan -> reload -> persisteert', async ({ page }) => {
  expect(traject, 'traject uit stap 3').not.toBeNull();

  await page.goto(`/trajecten/${traject.ref}/editor/${LAW_ID}/${ARTICLE}`);
  await expect(
    page.locator('nldd-document-tab-bar-item', { hasText: `Artikel ${ARTICLE}` }).first(),
  ).toBeVisible({ timeout: 60_000 });

  const initBtn = page.locator('[data-testid="init-mr-btn"]');
  // Precondition for this write smoke: article 1 has no machine_readable yet, so
  // the pane offers the "author a machine version" button. If a deployment's
  // corpus already ships machine_readable here, there is nothing to author -
  // skip rather than assert a false negative.
  if ((await initBtn.count()) === 0) {
    test.skip(true, `${LAW_ID} artikel ${ARTICLE} heeft al machine_readable in dit corpus - geen init-stap om te smoken`);
  }

  // Author an (empty) machine_readable scaffold: this dirties the article and
  // raises the Wijzigingenbalk.
  await initBtn.first().click();
  await expect(machineReadablePane(page).first()).toBeVisible();

  const changesBar = page.locator('[slot="changes-bar"]');
  const saveBtn = changesBar.locator('nldd-button[text="Opslaan"]').first();
  await expect(saveBtn).toBeVisible();

  // The save PUTs the whole law and commits+pushes to the corpus remote, so it
  // is slow - wait on the response, not a fixed timeout. nldd-button is a Lit
  // element; dispatch the click on the element itself (matches the e2e helpers).
  const putPromise = page.waitForResponse(
    (r) => r.request().method() === 'PUT' && r.url().includes(`/corpus/laws/${LAW_ID}`),
    { timeout: 150_000 },
  );
  await saveBtn.evaluate((el) => el.click());
  const put = await putPromise;

  // Some deployments require the acting user's OWN linked GitHub token for
  // traject writes and answer 428 (Precondition Required) when it is missing.
  // That is an environment precondition, not a regression in the editor - skip
  // the persistence assertion with an actionable reason.
  if (put.status() === 428) {
    test.skip(
      true,
      'Traject-write vereist een gekoppelde persoonlijke GitHub-token (428). ' +
        'Draai tegen een preview met de service-account-schrijfweg, of koppel de test-user in de UI.',
    );
  }
  expect(put.ok(), `PUT ${put.url()} -> ${put.status()}`).toBeTruthy();

  // The Wijzigingenbalk clears once the save settled.
  await expect(changesBar).toHaveCount(0, { timeout: 30_000 });

  // Persistence, two ways: the API now serves a machine_readable body...
  const lawYaml = await readTrajectLaw(page.request, traject.ref, LAW_ID);
  expect(lawYaml).toContain('machine_readable');

  // ...and a fresh page load shows the populated pane instead of the empty state.
  await page.goto(`/trajecten/${traject.ref}/editor/${LAW_ID}/${ARTICLE}`);
  await expect(
    page.locator('nldd-document-tab-bar-item', { hasText: `Artikel ${ARTICLE}` }).first(),
  ).toBeVisible({ timeout: 60_000 });
  await expect(machineReadablePane(page).first()).toBeVisible({ timeout: 30_000 });
  await expect(page.locator('[data-testid="no-machine-readable"]')).toHaveCount(0);
});
