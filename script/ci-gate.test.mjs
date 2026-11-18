// De `Test`-poort in .github/workflows/ci.yml is de enige required check die
// het testwerk afdekt. Hij is één shellfragment met een `case`, en die vorm
// faalt stil op twee manieren: een voorganger die wel in `needs` staat maar
// niet in het fragment wordt nooit gelezen, en een `case` die `skipped` niet
// als geslaagd behandelt laat elke PR omvallen die een baan overslaat.
//
// Deze test knipt het fragment uit de workflow, vult echte resultaatwaarden in
// en draait het. Node's ingebouwde runner, geen dependency: de Pre-commit-baan
// heeft Node maar geen node_modules.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const WORKFLOW = fileURLToPath(new URL('../.github/workflows/ci.yml', import.meta.url));
const source = readFileSync(WORKFLOW, 'utf8');
const lines = source.split('\n');

/** Regelnummers waarop een baan op het bovenste niveau van `jobs:` begint. */
function jobStarts() {
  const starts = new Map();
  let inJobs = false;
  for (const [index, line] of lines.entries()) {
    if (/^jobs:\s*$/.test(line)) { inJobs = true; continue; }
    if (!inJobs) continue;
    if (/^\S/.test(line)) break;
    const match = /^ {2}([A-Za-z0-9_-]+):\s*$/.exec(line);
    if (match) starts.set(match[1], index);
  }
  return starts;
}

const JOBS = jobStarts();

/** De regels van één baan, tot aan de volgende baan. */
function jobBody(name) {
  const start = JOBS.get(name);
  assert.ok(start !== undefined, `baan ${name} bestaat niet in ci.yml`);
  const next = [...JOBS.values()].filter((i) => i > start).sort((a, b) => a - b)[0];
  return lines.slice(start + 1, next ?? lines.length);
}

/** The names in a job's `needs:`, in all three YAML forms. */
function jobNeeds(job) {
  const body = jobBody(job);
  const index = body.findIndex((line) => /^ {4}needs:/.test(line));
  assert.ok(index !== -1, `${job} has no needs:`);

  const single = /^ {4}needs:\s*([A-Za-z0-9_-]+)\s*$/.exec(body[index]);
  if (single) return [single[1]];

  const inline = /^ {4}needs:\s*\[(.+)\]\s*$/.exec(body[index]);
  if (inline) return inline[1].split(',').map((n) => n.trim());

  const names = [];
  for (const line of body.slice(index + 1)) {
    if (/^\s*#/.test(line) || line.trim() === '') continue;
    const item = /^ {6}-\s*([A-Za-z0-9_-]+)\s*$/.exec(line);
    if (!item) break;
    names.push(item[1]);
  }
  return names;
}

/** Het shellfragment onder `run:` van de Gate-stap. */
function gateScript() {
  const body = jobBody('test');
  const index = body.findIndex((line) => /^ {8}run: \|\s*$/.test(line));
  assert.ok(index !== -1, 'de Gate-stap heeft geen `run: |`-blok');

  const script = [];
  for (const line of body.slice(index + 1)) {
    if (line.trim() !== '' && !line.startsWith('          ')) break;
    script.push(line.slice(10));
  }
  return script.join('\n');
}

const NEEDS = jobNeeds('test');
const SCRIPT = gateScript();

/**
 * Draai de poort met een resultaat per voorganger. `${{ needs.X.result }}`
 * wordt vervangen zoals Actions dat doet: platte tekstsubstitutie vóór de shell
 * het fragment ziet.
 */
function runGate(results) {
  const filled = SCRIPT.replace(/\$\{\{\s*needs\.([A-Za-z0-9_-]+)\.result\s*\}\}/g, (_, job) => {
    assert.ok(job in results, `geen resultaat opgegeven voor ${job}`);
    return results[job];
  });
  // Dezelfde shellvlaggen als de standaardshell van Actions (`bash -e -o
  // pipefail`), anders bewijst een groene test niets over de echte run.
  return spawnSync('bash', ['-e', '-o', 'pipefail', '-c', filled], { encoding: 'utf8' });
}

// De baan die bepaalt of de rest überhaupt draait. Hij hoort niet in de
// skipped-is-geslaagd-regel thuis en heeft daarom zijn eigen assertie.
const STRICT = 'changes';

/** Alle voorgangers op `value`, behalve `changes`: die blijft geslaagd. */
const withAll = (value) =>
  Object.fromEntries(NEEDS.map((job) => [job, job === STRICT ? 'success' : value]));

// Banen die zelf een verplichte check op main zijn, en dus niet aan de poort
// hoeven te hangen. Deze lijst moet overeenkomen met de required contexts in de
// branch protection; wijzigt daar iets, dan hier ook.
const EIGEN_REQUIRED_CHECK = new Set([
  'protect-schema', // Protect schema versions
  'pre-commit', //     Pre-commit
  'wasm', //           WASM Build
  'audit', //          Security Audit
  'test', //           Test, de poort zelf
]);

// Banen die bewust buiten beide vallen. Leeg, en dat hoort zo te blijven: elke
// regel hier is dekking die niemand meer bewaakt, dus zet er de reden bij.
const BUITEN_DE_POORT = new Set([]);

test('elke baan in ci.yml blokkeert, via de poort of als eigen required check', () => {
  // Zonder deze assertie is een nieuwe baan toevoegen en vergeten aan te haken
  // stil dekkingsverlies, en dat is precies de fout die de poort kwam
  // repareren.
  const ongedekt = [...JOBS.keys()].filter(
    (job) => !NEEDS.includes(job) && !EIGEN_REQUIRED_CHECK.has(job) && !BUITEN_DE_POORT.has(job),
  );
  assert.deepEqual(
    ongedekt,
    [],
    'deze banen hangen niet aan de poort en zijn zelf geen verplichte check, dus ze houden niets tegen',
  );
});

test('de lijsten met verplichte checks en uitzonderingen slaan op bestaande banen', () => {
  for (const job of [...EIGEN_REQUIRED_CHECK, ...BUITEN_DE_POORT]) {
    assert.ok(JOBS.has(job), `${job} staat in een lijst maar bestaat niet als baan in ci.yml`);
  }
});

test('de poort hangt aan de vier checks die tot nu toe niets blokkeerden', () => {
  for (const job of ['e2e', 'cross-law-integrity', 'provenance-checks', 'docs-a11y']) {
    assert.ok(NEEDS.includes(job), `${job} hangt niet aan de poort`);
    assert.ok(JOBS.has(job), `${job} bestaat niet als baan in ci.yml`);
  }
});

test('elke voorganger uit needs wordt in het fragment ook echt gelezen', () => {
  // Zonder deze assertie levert een vergeten regel een groene poort op voor een
  // baan die rood is.
  const read = new Set([...SCRIPT.matchAll(/needs\.([A-Za-z0-9_-]+)\.result/g)].map((m) => m[1]));
  for (const job of NEEDS) {
    assert.ok(read.has(job), `${job} staat in needs maar wordt niet gelezen door de poort`);
  }
  for (const job of read) {
    assert.ok(NEEDS.includes(job), `de poort leest ${job} maar heeft hem niet in needs`);
  }
});

test('alles geslaagd laat de poort door', () => {
  assert.equal(runGate(withAll('success')).status, 0);
});

test('alles overgeslagen laat de poort door', () => {
  // Een PR die alleen docs raakt slaat elke testbaan over. Zou skipped hier
  // rood worden, dan blokkeert de required check zo'n PR voorgoed.
  assert.equal(runGate(withAll('skipped')).status, 0);
});

test('de poort hangt aan changes en leest hem strikt', () => {
  assert.ok(NEEDS.includes(STRICT), `${STRICT} hangt niet aan de poort`);
  assert.ok(JOBS.has(STRICT), `${STRICT} bestaat niet als baan in ci.yml`);

  // Overgeslagen of gefaald hoort hier rood te zijn, ook al is skipped voor elke
  // andere voorganger geslaagd: de rest heeft dan niet gedraaid.
  for (const result of ['skipped', 'failure', 'cancelled']) {
    const run = runGate({ ...withAll('skipped'), [STRICT]: result });
    assert.equal(run.status, 1, `changes op ${result} liet de poort door`);
    assert.match(run.stdout, /::error::changes gaf/);
  }
});

test('een gevallen voorganger laat de poort omvallen, welke dan ook', () => {
  for (const job of NEEDS) {
    const result = runGate({ ...withAll('success'), [job]: 'failure' });
    assert.equal(result.status, 1, `${job} op failure liet de poort door`);
    assert.match(result.stdout, new RegExp(`::error::${job} gaf failure`));
  }
});

test('een afgebroken voorganger laat de poort omvallen, welke dan ook', () => {
  for (const job of NEEDS) {
    const result = runGate({ ...withAll('skipped'), [job]: 'cancelled' });
    assert.equal(result.status, 1, `${job} op cancelled liet de poort door`);
  }
});

test('een gevallen voorganger tussen overgeslagen banen valt niet weg', () => {
  const [first] = NEEDS;
  const result = runGate({ ...withAll('skipped'), [first]: 'failure' });
  assert.equal(result.status, 1);
});

test('every job that downloads the WASM engine needs wasm, and so does the gate', () => {
  // A job whose `needs` fails is skipped, and the gate reads skipped as
  // passed. Without `wasm` in the gate, a broken engine build would leave e2e
  // and walkthrough-replay skipped and the gate green.
  const downloadsEngine = (body) =>
    body.some((line, i) => /actions\/download-artifact@/.test(line)
      && body.slice(i + 1, i + 4).some((next) => /^\s+name: wasm-engine\s*$/.test(next)));
  const consumers = [...JOBS.keys()].filter((job) => downloadsEngine(jobBody(job)));
  assert.ok(consumers.length > 0, 'no job downloads the wasm-engine artifact');
  for (const job of consumers) {
    assert.ok(jobNeeds(job).includes('wasm'), `${job} downloads wasm-engine but does not need wasm`);
  }
  assert.ok(NEEDS.includes('wasm'), 'the gate does not need wasm');
});
