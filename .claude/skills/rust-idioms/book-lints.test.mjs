// Tests for the parts of book-lints.mjs that decide whether a finding is
// reported: argument parsing, the diff parser, range matching and the
// test-code check. Each case is one way the gate could wrongly stay green.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isTestCode, parseArgs, parseDiff, touches } from './book-lints.mjs';

const header = (file) => `diff --git a/${file} b/${file}\nindex 111..222 100644\n--- a/${file}\n+++ b/${file}\n`;

test('parseArgs splits its own options from cargo arguments', () => {
  assert.deepEqual(parseArgs(['--base', 'main', '-p', 'demo', '--all-targets']), {
    base: 'main',
    all: false,
    help: false,
    cargoArgs: ['-p', 'demo', '--all-targets'],
  });
  assert.equal(parseArgs(['--base=main']).base, 'main');
  assert.equal(parseArgs(['--all']).all, true);
  assert.equal(parseArgs(['--help']).help, true);
});

test('parseArgs rejects what it cannot honour', () => {
  assert.throws(() => parseArgs(['--base']), /needs a git ref/);
  assert.throws(() => parseArgs(['-p', 'demo', '--', '-W', 'clippy::all']), /not supported/);
});

test('parseDiff reads added and changed ranges per file', () => {
  const diff = `${header('src/a.rs')}@@ -3 +3,2 @@ fn a() {\n+one\n+two\n@@ -10,0 +12 @@\n+three\n${header('src/b.rs')}@@ -1 +1 @@\n-x\n+y\n`;
  const ranges = parseDiff(diff);
  assert.deepEqual(ranges.get('src/a.rs'), [[3, 4], [12, 12]]);
  assert.deepEqual(ranges.get('src/b.rs'), [[1, 1]]);
});

test('a hunk that only deletes lines touches the lines around the deletion', () => {
  const ranges = parseDiff(`${header('src/a.rs')}@@ -8 +7,0 @@\n-// SAFETY: gone\n`);
  assert.deepEqual(ranges.get('src/a.rs'), [[7, 8]]);
  assert.ok(touches(ranges, 'src/a.rs', 8, 8));
});

test('a file name with a space loses the tab git appends', () => {
  const diff = 'diff --git a/src/my mod.rs b/src/my mod.rs\n--- a/src/my mod.rs\t\n+++ b/src/my mod.rs\t\n@@ -1 +1 @@\n-x\n+y\n';
  assert.deepEqual([...parseDiff(diff).keys()], ['src/my mod.rs']);
});

test('an added line that looks like a file header stays content', () => {
  const diff = `${header('src/a.rs')}@@ -1,0 +2,2 @@\n+++ b/other.rs\n+x\n@@ -9 +11 @@\n-a\n+b\n`;
  const ranges = parseDiff(diff);
  assert.deepEqual([...ranges.keys()], ['src/a.rs']);
  assert.deepEqual(ranges.get('src/a.rs'), [[2, 3], [11, 11]]);
});

test('a deleted file contributes no ranges', () => {
  const diff = 'diff --git a/src/gone.rs b/src/gone.rs\ndeleted file mode 100644\n--- a/src/gone.rs\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-a\n-b\n';
  assert.equal(parseDiff(diff).size, 0);
});

test('touches compares inclusive ranges and knows only listed files', () => {
  const ranges = new Map([['src/a.rs', [[5, 7]]]]);
  assert.ok(touches(ranges, 'src/a.rs', 7, 9));
  assert.ok(touches(ranges, 'src/a.rs', 1, 5));
  assert.ok(!touches(ranges, 'src/a.rs', 8, 9));
  assert.ok(!touches(ranges, 'src/b.rs', 5, 7));
});

test('isTestCode: below a top-level test module, on one line or two', () => {
  const oneLine = 'fn a() {}\n#[cfg(test)] mod tests {\n    fn t() {}\n}\n';
  const twoLines = 'fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n';
  assert.ok(isTestCode('demo/src/lib.rs', 3, oneLine));
  assert.ok(isTestCode('demo/src/lib.rs', 4, twoLines));
  assert.ok(!isTestCode('demo/src/lib.rs', 1, twoLines));
});

test('isTestCode: a cfg(test) item that is no module leaves the rest production code', () => {
  const source = '#[cfg(test)]\nuse std::fmt;\n\n#[cfg(test)]\nfn helper() {}\n\nfn real() {}\n';
  assert.ok(!isTestCode('demo/src/lib.rs', 7, source));
});

test('isTestCode: files under tests/ or benches/ of the workspace', () => {
  assert.ok(isTestCode('demo/tests/cli.rs', 1, ''));
  assert.ok(isTestCode('demo\\benches\\speed.rs', 1, ''));
  assert.ok(!isTestCode('demo/src/tests_support.rs', 1, ''));
});
