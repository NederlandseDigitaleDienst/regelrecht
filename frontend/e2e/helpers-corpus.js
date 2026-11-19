/**
 * Shared corpus mock helper for Playwright e2e specs.
 *
 * Both `edit-test-loop.spec.js` (YAML-pane edit flow) and
 * `machine-panel-edit-loop.spec.js` (form-driven edit flow) need to fake
 * the editor-api's corpus endpoints so the spec can run without booting
 * Postgres + the Rust backend. Keeping the mock setup in one place
 * prevents the two specs from drifting and makes it cheap to add new
 * specs that exercise the same dependency graph.
 */
import * as yaml from 'js-yaml';
import { mockAuthedEditor } from './helpers.js';
import { loadCorpusVersions } from '../src/test-utils/corpusOnDisk.js';

export { CORPUS_ROOT, loadCorpus, loadCorpusVersions, loadScenario } from '../src/test-utils/corpusOnDisk.js';

// Corpus law endpoints exist in two shapes: the global read-only
// `/api/corpus/laws/...` and the authenticated, traject-scoped
// `/api/trajects/{ref}/corpus/laws/...` the editor actually reads through. This
// fragment matches the common `.../corpus/laws` tail of both, so one set of
// handlers serves either caller. The Playwright route globs below drop the
// `/api` prefix (`**` + `/corpus/laws...`) to match both for the same reason.
const LAWS_PATH = String.raw`\/api(?:\/trajects\/[^/]+)?\/corpus\/laws`;

/**
 * Set up route intercepts so the editor frontend can fetch any law in the
 * corpus without a running editor-api. Also stubs the scenarios list/get
 * and the PUT save endpoint.
 *
 * Playwright runs route handlers in reverse registration order (LIFO), so
 * the more specific `/api/corpus/laws/*` routes registered later take
 * precedence over the bare `/api/corpus/laws` list handler.
 *
 * @param {import('@playwright/test').Page} page
 * @param {Map<string, {content: string, path: string}>} corpus - from loadCorpus()
 * @param {{id: string, scenarioFilename: string}} scenarioLaw - which law's scenarios to expose
 * @param {string} scenarioFile - raw .feature text returned by the scenario fetch
 */
export async function mockCorpusApi(page, corpus, scenarioLaw, scenarioFile) {
  assertKeyedByLawId(corpus);

  // Authenticated `/auth/status` + `/api/trajects` list/detail so the
  // traject-scoped, `requiresAuth` editor route mounts. (Kept in the shared
  // helper so every corpus-driven spec gets the same signed-in shape.)
  await mockAuthedEditor(page);

  // GET .../corpus/laws - list for dependency discovery (fallback handler).
  await page.route('**/corpus/laws*', (route, request) => {
    const url = new URL(request.url());
    if (!new RegExp(`^${LAWS_PATH}$`).test(url.pathname)) {
      return route.fallback();
    }
    const entries = [...corpus.entries()].map(([law_id, entry]) => {
      const displayName = resolveDisplayName(entry.content);
      return {
        law_id,
        name: null,
        display_name: displayName,
        source_id: 'local',
        source_name: 'Local Test Corpus',
      };
    });
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(entries),
    });
  });

  // GET .../corpus/laws/{law_id}/outputs - list outputs from all articles.
  await page.route('**/corpus/laws/*/outputs', (route, request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(new RegExp(`${LAWS_PATH}\\/([^/]+)\\/outputs$`));
    if (!match) return route.fallback();
    const lawId = decodeURIComponent(match[1]);
    const entry = corpus.get(lawId);
    if (!entry) {
      return route.fulfill({ status: 404, body: `Law '${lawId}' not found` });
    }
    const outputs = collectOutputs(entry.content);
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(outputs),
    });
  });

  // GET .../corpus/laws/{law_id} - serve from the corpus map.
  // PUT .../corpus/laws/{law_id} - no-op; useLaw.saveLaw() updates rawYaml
  // locally on success so the test doesn't need a real backend write.
  await page.route('**/corpus/laws/*', (route, request) => {
    const url = new URL(request.url());
    const pathname = url.pathname;
    if (pathname.includes('/scenarios') || pathname.includes('/outputs')) {
      return route.fallback();
    }
    const lawId = decodeURIComponent(pathname.split('/').pop());
    if (request.method() === 'PUT') {
      return route.fulfill({ status: 200, body: '' });
    }
    const entry = corpus.get(lawId);
    if (!entry) {
      return route.fulfill({ status: 404, body: `Law '${lawId}' not found` });
    }
    return route.fulfill({
      status: 200,
      contentType: 'text/yaml; charset=utf-8',
      body: entry.content,
    });
  });

  // GET .../corpus/laws/{law_id}/scenarios - list, only for the target law.
  await page.route('**/corpus/laws/*/scenarios', (route, request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(new RegExp(`${LAWS_PATH}\\/([^/]+)\\/scenarios$`));
    if (!match) return route.fallback();
    const lawId = decodeURIComponent(match[1]);
    if (lawId === scenarioLaw.id) {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([{ filename: scenarioLaw.scenarioFilename }]),
      });
    }
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: '[]',
    });
  });

  // GET /api/corpus/laws/{law_id}/scenarios/{filename} - return the
  // scenario text for any law that asks. We only have one fixture so we
  // serve it for every match; tests that need multiple scenario files can
  // refine this later.
  await page.route('**/corpus/laws/*/scenarios/*', (route, request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(new RegExp(`${LAWS_PATH}\\/([^/]+)\\/scenarios\\/([^/]+)$`));
    if (!match) return route.fallback();
    return route.fulfill({
      status: 200,
      contentType: 'text/plain; charset=utf-8',
      body: scenarioFile,
    });
  });

  // GET .../corpus/laws/{law_id}/versions - the full version set the engine's
  // dependency loader pulls to run scenarios (cross-law resolution). Serve
  // every on-disk version for real-corpus specs; for a synthetic corpus map
  // (a spec that fabricates a law not on disk) fall back to that entry's single
  // content. Registered last so Playwright's LIFO order checks it before the
  // generic `/corpus/laws/*` handler.
  const allVersions = loadCorpusVersions();
  await page.route('**/corpus/laws/*/versions', (route, request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(new RegExp(`${LAWS_PATH}\\/([^/]+)\\/versions$`));
    if (!match) return route.fallback();
    const lawId = decodeURIComponent(match[1]);
    const versions = allVersions.get(lawId)
      ?? (corpus.has(lawId) ? [corpus.get(lawId).content] : []);
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(versions),
    });
  });

  // /api/sources - corpus source list (used by library page).
  await page.route('**/api/sources', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([
        { id: 'local', name: 'Local Test Corpus', source_type: 'local', priority: 1, law_count: corpus.size },
      ]),
    }),
  );
}

/**
 * Fail loudly when a corpus map keys a law under anything but its own `$id`.
 *
 * The backend cannot serve such a body: `SourceMap` indexes laws by `$id`, so
 * a fetch under another id is a 404. The editor takes the mismatch at face
 * value instead — `lawId` is `law.$id || routeParam`, so it silently switches
 * to the document's id once the YAML lands, and every follow-up call asks for
 * an id the mock does not serve. That surfaces as an unexplained 30-second
 * timeout on a missing element, so catch it here where the cause is still
 * visible.
 */
function assertKeyedByLawId(corpus) {
  for (const [key, entry] of corpus) {
    const idMatch = entry?.content?.match(/^\$id:\s*['"]?([^'"\n]+)['"]?$/m);
    const lawId = idMatch?.[1].trim();
    if (lawId && lawId !== key) {
      throw new Error(
        `mockCorpusApi: law keyed as '${key}' but its YAML declares $id '${lawId}'. ` +
        'Key the corpus map on the document\'s own $id.',
      );
    }
  }
}

/**
 * Resolve a law's display name from its YAML content. Mirrors the backend's
 * `resolve_display_name` logic: literal name → return it; `#ref` → find the
 * matching action output value.
 */
function resolveDisplayName(yamlContent) {
  try {
    const doc = yaml.load(yamlContent);
    if (!doc) return null;
    const name = doc.name;
    if (typeof name === 'string' && !name.startsWith('#')) return name;
    if (typeof name === 'string' && name.startsWith('#')) {
      const ref = name.slice(1);
      for (const article of doc.articles || []) {
        for (const action of article.machine_readable?.execution?.actions || []) {
          if (action.output === ref && typeof action.value === 'string') {
            return action.value;
          }
        }
      }
    }
  } catch { /* ignore parse errors */ }
  return null;
}

/**
 * Collect all outputs declared across all articles in a law's YAML.
 * Returns `[{ name, output_type, article_number }]`, deduplicated by name.
 */
function collectOutputs(yamlContent) {
  try {
    const doc = yaml.load(yamlContent);
    if (!doc?.articles) return [];
    const seen = new Set();
    const results = [];
    for (const article of doc.articles) {
      const exec = article.machine_readable?.execution;
      if (!exec) continue;
      const params = (exec.parameters || []).map(p => ({
        name: p.name,
        param_type: p.type || 'string',
      }));
      for (const out of exec.output || []) {
        if (out.name && !seen.has(out.name)) {
          seen.add(out.name);
          results.push({
            name: out.name,
            output_type: out.type || 'string',
            article_number: String(article.number || ''),
            parameters: params,
          });
        }
      }
    }
    results.sort((a, b) => a.name.localeCompare(b.name));
    return results;
  } catch { return []; }
}
