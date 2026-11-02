// Fails when a test exists that no CI job runs: a JS or shell test no step,
// hook, recipe or npm script reaches, a crate no `cargo test` covers, a
// feature file no BDD runner in CI reads, or a `@wip` / `#[ignore]` without a
// reason. The rules and why they exist: script/orphan-tests.mjs.
//
//   node script/check-orphan-tests.mjs

import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { findOrphans, trackedFiles } from './orphan-tests.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const findings = findOrphans(root, trackedFiles(root));

for (const { rule, file, line, message } of findings) {
  console.error(`✗ [${rule}] ${file}${line ? `:${line}` : ''}: ${message}`);
}
if (findings.length > 0) {
  console.error(
    `\n${findings.length} finding${findings.length === 1 ? '' : 's'}. ` +
      'Wire the test into CI (a pre-commit hook, a workflow step, or a recipe one of them calls), ' +
      'or give the skip its reason.',
  );
  process.exit(1);
}
console.log('✓ every test runs somewhere in CI, and every skip says why');
