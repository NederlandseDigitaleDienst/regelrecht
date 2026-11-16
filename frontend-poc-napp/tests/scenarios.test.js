// Draait elk Gherkin-scenario uit corpus-poc/napp/scenarios/ (de getoonde én
// de randgevallen) door de JS-runner die de beoordelaarsview gebruikt, tegen
// de WASM-engine met de napp-wetten. Een mislukte of onbekende stap maakt de
// test rood.
//
// De Rust-runner in packages/poc-napp/tests/bdd leest dezelfde map. Deze test
// bewaakt iets anders: de JS-stapdefinities in src/gherkin/steps.js, die de
// view in de browser draait en die verder door niets getoetst worden.
import { describe, it, expect, beforeAll } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'fs';
import { resolve, dirname, relative } from 'path';
import { fileURLToPath } from 'url';
import { parseFeature } from '../src/gherkin/parser.js';
import { runSteps } from '../src/gherkin/runner.js';
import { laadWetten } from '../src/engine.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '..', '..');
const scenariosDir = resolve(repoRoot, 'corpus-poc', 'napp', 'scenarios');
// Waar `just wasm-build` de engine neerzet; CI downloadt hem naar dezelfde plek.
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
  // Eén engine voor alle scenario's, zoals in de view: de stappen roepen
  // alleen executeMultiple aan en laten geen toestand achter.
  engine = laadWetten(new wasm.WasmEngine());
});

it('vindt de getoonde scenario\'s en de randgevallen', () => {
  const mappen = new Set(featureFiles.map((f) => relative(scenariosDir, dirname(f))));
  expect([...mappen].sort()).toEqual(['getoond', 'randgevallen']);
});

describe.each(featureFiles.map((f) => [relative(scenariosDir, f), f]))('%s', (_label, file) => {
  const { background, scenarios } = parseFeature(readFileSync(file, 'utf-8'));

  it.each(scenarios.map((s) => [s.name, s]))('%s', async (_name, scenario) => {
    const uitkomst = await runSteps([...(background ?? []), ...scenario.steps], engine);
    const mislukt = uitkomst.steps.find((s) => s.status === 'mislukt');
    expect(mislukt ? `${mislukt.text}: ${mislukt.error}` : null).toBeNull();
    expect(uitkomst.passed).toBe(true);
  });
});
