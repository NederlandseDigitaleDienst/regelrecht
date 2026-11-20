#!/usr/bin/env node
/**
 * Construct mutation: break one construct of the law format at a time in the
 * synthetic `test_*` regelwerken and run the language suites (BDD bucket B
 * and the cell's suite). A construct is pinned when a scenario turns red.
 *
 * Line coverage says a construct ran. This says whether a scenario would
 * notice if it did something else, which is the question a test answers.
 * The mutations themselves live in
 * `packages/engine/tests/common/construct_mutation.rs`; this script only
 * runs the suites once per construct and writes the report.
 *
 *   node script/construct-mutants.mjs [--out report.md] [--check <gaps file>]
 *                                     [--only NAME[,NAME...]] [--jobs N]
 *                                     [--shard k/n] [--json results.json]
 *   node script/construct-mutants.mjs --merge a.json b.json [--out ...] [--check ...]
 *
 * `--check` compares the outcome with a list of known gaps: a construct that
 * stops being pinned fails the run, and so does one on the list that has
 * since become pinned, so the list only gets shorter.
 */
import { spawn, spawnSync } from 'node:child_process';
import os from 'node:os';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');

/** The two suites, as `cargo test` arguments. */
export const SUITES = [
  { name: 'conformance', args: ['-p', 'regelrecht-engine', '--test', 'bdd'], env: { BDD_BUCKET: 'conformance' } },
  { name: 'cel', args: ['-p', 'regelrecht-cel', '--test', 'bdd'], env: {} },
];

/** A construct without a site in any synthetic regelwerk. */
export const ABSENT = 'absent';
export const PINNED = 'pinned';
export const UNPINNED = 'unpinned';

/**
 * What one suite run says: the sites the loader mutated (per file, since a
 * file loads once per feature) and the scenarios with a failing step.
 */
export function parseRun(output) {
  const sites = new Map();
  const failed = new Set();
  let scenario = null;
  for (const line of output.split('\n')) {
    const site = line.match(/construct-mutation: (\d+) site\(s\) in (.+)$/);
    if (site) {
      sites.set(site[2].trim(), Number(site[1]));
      continue;
    }
    const heading = line.match(/^\s*(?:Scenario|Scenario Outline|Example):\s*(.+?)\s*$/);
    if (heading) {
      scenario = heading[1];
      continue;
    }
    if (scenario && line.includes('✘')) failed.add(scenario);
  }
  return { sites, failed };
}

/** The outcome for one construct from the runs of all suites. */
export function outcome(runs) {
  const sites = new Map();
  for (const run of runs) for (const [file, n] of run.sites) sites.set(file, n);
  const total = [...sites.values()].reduce((a, b) => a + b, 0);
  const pinnedBy = runs.filter((r) => !r.ok).map((r) => r.suite);
  const scenarios = runs.flatMap((r) => [...r.failed]);
  let status;
  if (total === 0) status = ABSENT;
  else if (pinnedBy.length > 0) status = PINNED;
  else status = UNPINNED;
  return { status, sites: total, files: sites.size, pinnedBy, scenarios };
}

/** The known gaps: one construct name per line, `#` starts a comment. */
export function parseGaps(text) {
  return new Set(
    text
      .split('\n')
      .map((l) => l.replace(/#.*/, '').trim())
      .filter(Boolean),
  );
}

/**
 * Compare results with the known gaps. `known` holds every construct name the
 * runner has, also those this run skipped (`--only`). Returns the problems,
 * each a sentence; an empty list is green.
 */
export function compare(results, gaps, known = results.map((r) => r.name)) {
  const problems = [];
  const names = new Set(known);
  for (const r of results) {
    const gap = r.status !== PINNED;
    if (gap && !gaps.has(r.name)) {
      problems.push(
        `${r.name} is ${r.status === ABSENT ? 'in no synthetic regelwerk' : 'not pinned by any scenario'}, ` +
          'and it is not a known gap. Add a scenario whose assertion depends on it.',
      );
    }
    if (!gap && gaps.has(r.name)) {
      problems.push(`${r.name} is pinned now. Remove it from the known gaps.`);
    }
  }
  for (const name of gaps) {
    if (!names.has(name)) problems.push(`${name} is a known gap but no construct of that name exists.`);
  }
  return problems;
}

const LABEL = {
  [PINNED]: 'vastgepind',
  [UNPINNED]: '**niet vastgepind**',
  [ABSENT]: '**komt niet voor**',
};

/** The report as markdown. */
export function report(results, seconds) {
  const count = (s) => results.filter((r) => r.status === s).length;
  const lines = [
    `Constructies met een mutatie: ${results.length}. Vastgepind (een scenario wordt rood als de ` +
      `constructie iets anders doet): **${count(PINNED)}**. Niet vastgepind: **${count(UNPINNED)}**. ` +
      `In geen enkel synthetisch regelwerk: **${count(ABSENT)}**.`,
    '',
    '| Constructie | Mutatie | Plekken | Uitkomst | Rood in |',
    '| --- | --- | ---: | --- | --- |',
  ];
  const order = { [ABSENT]: 0, [UNPINNED]: 1, [PINNED]: 2 };
  const sorted = [...results].sort((a, b) => order[a.status] - order[b.status] || a.name.localeCompare(b.name));
  for (const r of sorted) {
    const n = r.scenarios.length;
    const where = r.status === PINNED ? `${r.pinnedBy.join(', ')} (${n} ${n === 1 ? 'scenario' : "scenario's"})` : '';
    lines.push(`| \`${r.name}\` | ${r.how} | ${r.sites} | ${LABEL[r.status]} | ${where} |`);
  }
  if (seconds !== undefined) lines.push('', `Duur: ${Math.round(seconds / 60)} minuten.`);
  return lines.join('\n') + '\n';
}

/** Run one suite, with `mutation` broken (or nothing when null). */
function cargoTest(suite, mutation) {
  const env = { ...process.env, ...suite.env };
  if (mutation) env.BDD_CONSTRUCT_MUTATION = mutation;
  else delete env.BDD_CONSTRUCT_MUTATION;
  const args = ['test', '--manifest-path', path.join(root, 'packages/Cargo.toml'), ...suite.args, '--', '--nocapture'];
  return new Promise((resolve, reject) => {
    // Its own process group, so the timeout below can kill the test binary
    // that cargo starts and not only cargo: the binary holds the pipes open,
    // and 'close' would otherwise never come.
    const child = spawn('cargo', args, { env, detached: true });
    const chunks = [];
    child.stdout.on('data', (c) => chunks.push(c));
    child.stderr.on('data', (c) => chunks.push(c));
    // A mutation that makes a scenario loop forever counts as noticed, like a
    // timeout in cargo-mutants, but the run has to end.
    const timer = setTimeout(() => {
      try {
        process.kill(-child.pid, 'SIGKILL');
      } catch {
        child.kill('SIGKILL');
      }
    }, 20 * 60 * 1000);
    child.on('close', (code) => {
      clearTimeout(timer);
      const output = Buffer.concat(chunks).toString('utf8');
      if (output.includes('Failed to mutate')) {
        // The mutation itself broke, not a scenario: that is a bug in
        // construct_mutation.rs and must not count as a pin.
        process.stderr.write(output);
        reject(new Error(`${mutation}: the mutation could not be applied`));
        return;
      }
      resolve({ suite: suite.name, ok: code === 0, output, ...parseRun(output) });
    });
  });
}

/** Run `tasks` (functions returning promises) with at most `limit` at once. */
export async function pool(tasks, limit) {
  const results = new Array(tasks.length);
  let next = 0;
  const worker = async () => {
    while (next < tasks.length) {
      const i = next++;
      results[i] = await tasks[i]();
    }
  };
  await Promise.all(Array.from({ length: Math.max(1, Math.min(limit, tasks.length)) }, worker));
  return results;
}

function listConstructs() {
  const env = { ...process.env, BDD_CONSTRUCT_MUTATION: 'list' };
  const result = spawnSync(
    'cargo',
    ['test', '--manifest-path', path.join(root, 'packages/Cargo.toml'), '-p', 'regelrecht-engine', '--test', 'bdd'],
    { env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
  );
  if (result.status !== 0) throw new Error(`listing the constructs failed:\n${result.stderr}`);
  return result.stdout
    .split('\n')
    .filter((l) => l.includes('\t'))
    .map((l) => {
      const [name, how] = l.split('\t');
      return { name, how };
    });
}

/** The constructs of shard `k` of `n` (`"k/n"`), round-robin. */
export function shard(constructs, spec) {
  if (!spec) return constructs;
  const [k, n] = spec.split('/').map(Number);
  if (!(n > 0 && k >= 0 && k < n)) throw new Error(`--shard ${spec}: expected k/n with 0 <= k < n`);
  return constructs.filter((_, i) => i % n === k);
}

/** Break each construct in turn and return the results. */
async function measure(constructs, jobs) {
  const baselines = await pool(SUITES.map((s) => () => cargoTest(s, null)), jobs);
  for (const b of baselines) {
    if (!b.ok) {
      process.stderr.write(b.output);
      throw new Error(`the ${b.suite} suite is red without any mutation; fix that first`);
    }
  }
  const tasks = constructs.flatMap((c) => SUITES.map((s) => () => cargoTest(s, c.name)));
  const runs = await pool(tasks, jobs);
  return constructs.map((c, i) => {
    const r = { ...c, ...outcome(runs.slice(i * SUITES.length, (i + 1) * SUITES.length)) };
    process.stderr.write(`${c.name}: ${r.status} (${r.sites} sites)\n`);
    return r;
  });
}

/**
 * Two ways in. Measuring runs the suites, for every construct or for the
 * `--only` or `--shard` part; `--json` keeps the results for a later merge.
 * `--merge a.json b.json ...` reads the shards back instead. Either way the
 * report goes to stdout (and `--out`, and the job summary), and `--check`
 * compares with the known gaps.
 */
async function main(argv) {
  const arg = (flag) => {
    const i = argv.indexOf(flag);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  const started = Date.now();
  let results;
  let known;
  let seconds;
  if (argv.includes('--merge')) {
    const rest = argv.slice(argv.indexOf('--merge') + 1);
    const end = rest.findIndex((a) => a.startsWith('--'));
    const files = end < 0 ? rest : rest.slice(0, end);
    const shards = files.map((f) => JSON.parse(fs.readFileSync(f, 'utf8')));
    results = shards.flatMap((sh) => sh.results);
    known = [...new Set(shards.flatMap((sh) => sh.known))];
  } else {
    const all = listConstructs();
    known = all.map((c) => c.name);
    let constructs = shard(all, arg('--shard'));
    const only = arg('--only');
    if (only) {
      const wanted = new Set(only.split(','));
      const unknown = [...wanted].filter((name) => !known.includes(name));
      if (unknown.length > 0) throw new Error(`--only: no construct named ${unknown.join(', ')}`);
      constructs = constructs.filter((c) => wanted.has(c.name));
    }
    // The scenarios run one at a time, so a suite uses one core: run as many
    // suites side by side as there are cores. The binaries are built once.
    results = await measure(constructs, Number(arg('--jobs') ?? os.availableParallelism()));
    seconds = (Date.now() - started) / 1000;
    const json = arg('--json');
    if (json) fs.writeFileSync(json, JSON.stringify({ known, results }, null, 2));
  }

  const markdown = report(results, seconds);
  process.stdout.write(markdown);
  const out = arg('--out');
  if (out) fs.writeFileSync(out, markdown);
  if (process.env.GITHUB_STEP_SUMMARY) {
    fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, `## Constructiemutatie\n\n${markdown}`);
  }

  const gapsFile = arg('--check');
  if (gapsFile) {
    const problems = compare(results, parseGaps(fs.readFileSync(gapsFile, 'utf8')), known);
    for (const p of problems) process.stderr.write(`::error::${p}\n`);
    if (problems.length > 0) process.exit(1);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main(process.argv.slice(2)).catch((e) => {
    process.stderr.write(`${e.message}\n`);
    process.exit(2);
  });
}
