/**
 * The parts of the construct mutation run that decide green or red: reading a
 * suite's output, the outcome per construct, and the comparison with the
 * known gaps. The cargo runs themselves are not exercised here.
 */
import { strict as assert } from 'node:assert';
import { test } from 'node:test';

import { ABSENT, PINNED, UNPINNED, compare, outcome, parseGaps, parseRun, pool, report, shard } from './construct-mutants.mjs';

const OUTPUT = `
construct-mutation: 2 site(s) in /repo/corpus/regulation/nl/wet/test_a/2025-01-01.yaml
Feature: Something
  Scenario: The first one holds
   ✔  Then output "x" equals 1
  Scenario: The second one breaks
   ✘  Then output "y" equals 2
construct-mutation: 2 site(s) in /repo/corpus/regulation/nl/wet/test_a/2025-01-01.yaml
construct-mutation: 1 site(s) in /repo/packages/cel/bdd/fixtures/test_b.yaml
`;

test('a run counts each file once and names the scenarios with a failing step', () => {
  const run = parseRun(OUTPUT);
  assert.equal(run.sites.size, 2);
  assert.equal(run.sites.get('/repo/corpus/regulation/nl/wet/test_a/2025-01-01.yaml'), 2);
  assert.deepEqual([...run.failed], ['The second one breaks']);
});

const run = (suite, ok, sites, failed = []) => ({
  suite,
  ok,
  sites: new Map(Object.entries(sites)),
  failed: new Set(failed),
});

test('a construct a red suite noticed is pinned, and the suites that noticed are named', () => {
  const o = outcome([run('conformance', false, { a: 2 }, ['S']), run('cel', true, { b: 1 })]);
  assert.equal(o.status, PINNED);
  assert.equal(o.sites, 3);
  assert.deepEqual(o.pinnedBy, ['conformance']);
  assert.deepEqual(o.scenarios, ['S']);
});

test('a construct that was changed while every suite stayed green is unpinned', () => {
  assert.equal(outcome([run('conformance', true, { a: 1 }), run('cel', true, {})]).status, UNPINNED);
});

test('a construct without a site is absent, even when a suite failed for another reason', () => {
  assert.equal(outcome([run('conformance', false, {}), run('cel', true, {})]).status, ABSENT);
});

test('known gaps ignore comments and blank lines', () => {
  assert.deepEqual([...parseGaps('# header\nADD   # why\n\nIF\n')], ['ADD', 'IF']);
});

const result = (name, status) => ({ name, how: 'x', status, sites: 1, files: 1, pinnedBy: [], scenarios: [] });

test('a new gap is red', () => {
  const problems = compare([result('ADD', UNPINNED)], new Set());
  assert.equal(problems.length, 1);
  assert.match(problems[0], /ADD is not pinned/);
});

test('an absent construct that is no known gap is red too', () => {
  assert.match(compare([result('IF', ABSENT)], new Set())[0], /IF is in no synthetic regelwerk/);
});

test('a known gap that is pinned now must leave the list', () => {
  assert.match(compare([result('ADD', PINNED)], new Set(['ADD']))[0], /ADD is pinned now/);
});

test('a known gap that names no construct is red', () => {
  assert.match(compare([], new Set(['GONE']), [])[0], /GONE is a known gap but no construct/);
});

test('a known gap of a construct this run skipped is fine', () => {
  assert.deepEqual(compare([result('ADD', PINNED)], new Set(['IF']), ['ADD', 'IF']), []);
});

test('pinned constructs and listed gaps are green', () => {
  assert.deepEqual(compare([result('ADD', PINNED), result('IF', UNPINNED)], new Set(['IF'])), []);
});

test('the report lists gaps before pinned constructs', () => {
  const md = report([result('ADD', PINNED), result('IF', UNPINNED)]);
  assert.ok(md.indexOf('`IF`') < md.indexOf('`ADD`'));
  assert.match(md, /Vastgepind .*: \*\*1\*\*/);
});

test('the pool keeps the order of its tasks and never runs more than its limit', async () => {
  let running = 0;
  let peak = 0;
  const task = (value, ms) => async () => {
    running++;
    peak = Math.max(peak, running);
    await new Promise((r) => setTimeout(r, ms));
    running--;
    return value;
  };
  const results = await pool([task('a', 20), task('b', 5), task('c', 10), task('d', 1)], 2);
  assert.deepEqual(results, ['a', 'b', 'c', 'd']);
  assert.equal(peak, 2);
});

test('the shards split the constructs without overlap or loss', () => {
  const all = ['a', 'b', 'c', 'd', 'e'];
  const parts = [0, 1, 2].map((k) => shard(all, `${k}/3`));
  assert.deepEqual(parts.flat().sort(), all);
  assert.deepEqual(parts[0], ['a', 'd']);
  assert.deepEqual(shard(all, undefined), all);
  assert.throws(() => shard(all, '3/3'), /expected k\/n/);
});
