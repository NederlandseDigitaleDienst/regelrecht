/**
 * The law corpus read straight off disk, one version per `$id` the way the
 * editor-api serves it. The Playwright specs build their corpus mock from it
 * (`e2e/helpers-corpus.js`); it lives outside `e2e/` so its vitest test does
 * not sit among the Playwright specs.
 */
import { readFileSync, readdirSync, statSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { compareVersionsNewestFirst, pickBestVersion, todayIso, extractDateFromPath } from './corpusVersion.js';

const __dirname = dirname(fileURLToPath(import.meta.url));

/**
 * Absolute path to the on-disk corpus the helper reads law fixtures from.
 * Resolved relative to this file so callers don't need to know about it.
 */
export const CORPUS_ROOT = resolve(__dirname, '../../../corpus/regulation/nl');

/**
 * Recursively walk a corpus directory and pick one YAML per `$id` — the version
 * the editor-api would serve today, per `SourceMap::pick_best_version`. Returns
 * a Map<lawId, { content, path }>.
 *
 * `today` is injectable so a test can pin the "in force now" boundary; it
 * defaults to the real date, which is what the specs run on.
 */
export function loadCorpus(rootDir = CORPUS_ROOT, today = todayIso()) {
  const byId = new Map();

  function visit(dir) {
    for (const entry of readdirSync(dir)) {
      const full = resolve(dir, entry);
      if (statSync(full).isDirectory()) {
        visit(full);
      } else if (entry.endsWith('.yaml')) {
        const content = readFileSync(full, 'utf-8');
        const idMatch = content.match(/^\$id:\s*['"]?([^'"\n]+)['"]?$/m);
        if (!idMatch) continue;
        const lawId = idMatch[1].trim();
        const existing = byId.get(lawId);
        const wins = !existing || pickBestVersion(
          extractDateFromPath(existing.path),
          extractDateFromPath(full),
          today,
        );
        if (wins) byId.set(lawId, { content, path: full });
      }
    }
  }

  visit(rootDir);
  return byId;
}

/**
 * Recursively walk a corpus directory and collect **every** version YAML per
 * `$id`, newest filename date first (undated last) — the order the editor-api's
 * `/versions` endpoint returns. Returns Map<lawId, string[]>.
 *
 * The engine's dependency loader fetches `/corpus/laws/{id}/versions` and loads
 * the whole version set so its date-aware resolver can pick the one in force on
 * the scenario's calculation date. Returning only a single picked version would
 * make a scenario dated before the latest file fail to resolve, so the versions
 * mock must serve all of them.
 */
export function loadCorpusVersions(rootDir = CORPUS_ROOT) {
  const byId = new Map();

  function visit(dir) {
    for (const entry of readdirSync(dir)) {
      const full = resolve(dir, entry);
      if (statSync(full).isDirectory()) {
        visit(full);
      } else if (entry.endsWith('.yaml')) {
        const content = readFileSync(full, 'utf-8');
        const idMatch = content.match(/^\$id:\s*['"]?([^'"\n]+)['"]?$/m);
        if (!idMatch) continue;
        const lawId = idMatch[1].trim();
        const list = byId.get(lawId) ?? [];
        list.push({ content, path: full });
        byId.set(lawId, list);
      }
    }
  }

  visit(rootDir);
  for (const [id, list] of byId) {
    list.sort((a, b) => compareVersionsNewestFirst(a.path, b.path));
    byId.set(id, list.map((e) => e.content));
  }
  return byId;
}

/**
 * Find a scenario .feature file next to a law's YAML on disk. Returns the
 * raw text or null when no scenarios directory exists for that law.
 */
export function loadScenario(lawPath, filename) {
  const scenariosDir = resolve(dirname(lawPath), 'scenarios');
  try {
    return readFileSync(resolve(scenariosDir, filename), 'utf-8');
  } catch {
    return null;
  }
}
