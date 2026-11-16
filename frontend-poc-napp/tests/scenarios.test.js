// Runs every Gherkin scenario under corpus-poc/napp/scenarios/ (the shown
// ones and the edge cases) through the JS runner the beoordelaar view uses,
// against the WASM engine with the napp laws. A failed or undefined step
// turns the test red.
//
// The Rust runner in packages/poc-napp/tests/bdd reads the same directory.
// This test guards something else: the JS step definitions in
// src/gherkin/steps.js, which the view runs in the browser and nothing else
// exercises.
import { describe, it, expect, beforeAll } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'fs';
import { resolve, dirname, relative } from 'path';
import { fileURLToPath } from 'url';
import { parseFeature } from '../src/gherkin/parser.js';
import { runSteps } from '../src/gherkin/runner.js';
import { loadLaws } from '../src/engine.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '..', '..');
const scenariosDir = resolve(repoRoot, 'corpus-poc', 'napp', 'scenarios');
// Where `just wasm-build` puts the engine; CI downloads it to the same place.
const pkgDir = resolve(repoRoot, 'frontend', 'public', 'wasm', 'pkg');

function findFeatureFiles(dir) {
  const results = [];
  for (const entry of readdirSync(dir)) {
    const full = resolve(dir, entry);
    if (statSync(full).isDirectory()) results.push(...findFeatureFiles(full));
    else if (entry.endsWith('.feature')) results.push(full);
  }
  return results.sort();
}

const featureFiles = findFeatureFiles(scenariosDir);

let engine;

beforeAll(async () => {
  const wasm = await import(resolve(pkgDir, 'regelrecht_engine.js'));
  await wasm.default({ module_or_path: readFileSync(resolve(pkgDir, 'regelrecht_engine_bg.wasm')) });
  // One engine for every scenario, as in the view: the steps only call
  // executeMultiple and leave no state behind.
  engine = loadLaws(new wasm.WasmEngine());
});

// The view globs getoond/; renaming that directory would leave it empty
// without an error, so pin both directories here.
it('finds the shown scenarios and the edge cases', () => {
  const dirs = new Set(featureFiles.map((f) => relative(scenariosDir, dirname(f))));
  expect([...dirs].sort()).toEqual(['getoond', 'randgevallen']);
});

describe.each(featureFiles.map((f) => [relative(scenariosDir, f), f]))('%s', (_label, file) => {
  const { background, scenarios } = parseFeature(readFileSync(file, 'utf-8'));

  it.each(scenarios.map((s) => [s.name, s]))('%s', async (_name, scenario) => {
    const outcome = await runSteps([...(background ?? []), ...scenario.steps], engine);
    const failed = outcome.steps.find((s) => s.status === 'mislukt');
    expect(failed ? `${failed.text}: ${failed.error}` : null).toBeNull();
    expect(outcome.passed).toBe(true);
  });
});
