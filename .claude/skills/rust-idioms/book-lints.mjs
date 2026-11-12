#!/usr/bin/env node
// The gate of the rust-idioms skill: clippy, restricted to the lints that
// correspond to a rule in the skill, reported only for lines you changed.
//
//   node book-lints.mjs [--base <git-ref>] [--all] [cargo clippy arguments...]
//
//   --base <ref>  report lines changed since the point where HEAD branched off
//                 <ref> (default: HEAD, so uncommitted changes only). In a
//                 repository without commits every line counts as changed.
//   --all         report every finding, changed or not (a new crate, a review).
//   anything else is passed to `cargo clippy` (-p <crate>, --all-targets, ...).
//
// Run it from the directory you run cargo in. Exit status: 0 no findings,
// 1 findings, 2 the tool itself failed.
//
// Filtering on changed lines is what makes this usable in an existing
// codebase: the lints are stricter than a project's own set, and a crate that
// predates the skill carries findings that are not yours to fix in passing.
//
// What it cannot see: code clippy did not compile. Tests need `--all-targets`,
// another crate needs its own `-p`, and code behind a feature needs
// `--features`. A function that was already over the length limit is not
// reported when you make it longer; only a new or re-signed function is.

import { spawn, spawnSync } from 'node:child_process';
import { createInterface } from 'node:readline';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Lint -> the rule it stands for. A lint is listed only when a rule in the
// skill says the same thing; this is not a general strictness setting.
const LINTS = {
  'clippy::needless_pass_by_value': 'a parameter borrows unless the function consumes it (SKILL.md, Signatures)',
  'clippy::ptr_arg': 'text comes in as `&str`, not `&String` (SKILL.md, Signatures)',
  'clippy::redundant_clone': 'every `.clone()` is a decision (SKILL.md, Signatures)',
  'clippy::unwrap_used': 'state the reason it cannot fail, or propagate (SKILL.md, Errors)',
  'clippy::let_underscore_must_use': 'every `Result` is handled or propagated (SKILL.md, Errors)',
  'clippy::manual_let_else': '`let ... else` for the precondition (SKILL.md, Control flow)',
  'clippy::too_many_lines': 'one idea per function (SKILL.md, Structure)',
  'clippy::wildcard_imports': 'name what you import (SKILL.md, Structure)',
  'clippy::uninlined_format_args': 'format arguments go inline (SKILL.md, Structure)',
  'clippy::missing_errors_doc': '`# Errors` on public functions that return `Result` (SKILL.md, Docs)',
  'clippy::missing_panics_doc': '`# Panics` on public functions that can panic (SKILL.md, Docs)',
  'clippy::missing_safety_doc': '`# Safety` on a public `unsafe fn` (unsafe.md, Shape)',
  'clippy::undocumented_unsafe_blocks': 'a `// SAFETY:` comment on every unsafe block (unsafe.md, Shape)',
  'unsafe_op_in_unsafe_fn': 'an `unsafe` block per operation inside an `unsafe fn` (unsafe.md, Shape)',
  'clippy::significant_drop_in_scrutinee': 'bind with `let` before the body; the guard lives through it (sharing-and-concurrency.md, Threads)',
  'clippy::significant_drop_tightening': 'release the guard as soon as the shared value is no longer needed (sharing-and-concurrency.md, Threads)',
  'clippy::mutex_atomic': 'an atomic for a shared number (sharing-and-concurrency.md, Threads)',
  'clippy::clone_on_ref_ptr': '`Rc::clone(&a)`, so it reads differently from a deep copy (sharing-and-concurrency.md, Shared ownership)',
  'clippy::should_panic_without_expect': '`#[should_panic(expected = "...")]` (testing.md, Assertions)',
};

// rustc reports this lint under its error code.
const CODE_ALIASES = { E0133: 'unsafe_op_in_unsafe_fn' };

// Lints that hold for production code only: the book is fine with `unwrap`
// in a test, where a panic is how the test fails (ch09-03).
const PRODUCTION_ONLY_LINTS = new Set(['clippy::unwrap_used']);

const USAGE = 'usage: node book-lints.mjs [--base <git-ref>] [--all] [cargo clippy arguments...]';

function fail(message) {
  console.error(`book-lints: ${message}`);
  process.exit(2);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, ...options });
  if (result.error) fail(`could not run ${command}: ${result.error.message}`);
  if (result.status !== 0) fail(`${command} ${args.join(' ')} failed:\n${result.stderr.trim()}`);
  return result.stdout;
}

export function parseArgs(argv) {
  const options = { base: 'HEAD', all: false, help: false, cargoArgs: [] };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      options.help = true;
    } else if (arg === '--base') {
      i += 1;
      if (argv[i] === undefined) throw new Error('--base needs a git ref');
      options.base = argv[i];
    } else if (arg.startsWith('--base=')) {
      options.base = arg.slice('--base='.length);
    } else if (arg === '--all') {
      options.all = true;
    } else if (arg === '--') {
      throw new Error('`--` is not supported: the lint flags after it are set by this script');
    } else {
      options.cargoArgs.push(arg);
    }
  }
  return options;
}

// Parses `git diff --unified=0` output (with the a/ and b/ prefixes) into
// a map of file -> [first, last] line ranges in the new version. A hunk that
// only deletes lines counts as touching the two lines around the deletion.
export function parseDiff(diff) {
  const ranges = new Map();
  let file = null;
  let inHeader = false;
  for (const raw of diff.split('\n')) {
    if (raw.startsWith('diff --git ')) {
      inHeader = true;
      file = null;
      continue;
    }
    if (inHeader && raw.startsWith('+++ ')) {
      // git appends a tab to a name that contains a space.
      const name = raw.slice(4).replace(/\t$/, '');
      file = name === '/dev/null' ? null : name.replace(/^b\//, '');
      continue;
    }
    const hunk = /^@@ -\S+ \+(\d+)(?:,(\d+))? @@/.exec(raw);
    if (!hunk) continue;
    inHeader = false;
    if (file === null) continue;
    const first = Number(hunk[1]);
    const count = hunk[2] === undefined ? 1 : Number(hunk[2]);
    if (!ranges.has(file)) ranges.set(file, []);
    ranges.get(file).push(count > 0 ? [first, first + count - 1] : [Math.max(first, 1), first + 1]);
  }
  return ranges;
}

export function touches(ranges, file, first, last) {
  return (ranges.get(file) ?? []).some(([a, b]) => first <= b && last >= a);
}

// Whether a line is test code: a file under a tests/ or benches/ directory of
// the workspace, or a line below a top-level `#[cfg(test)] mod`.
export function isTestCode(relativeFile, line, source) {
  if (relativeFile.split(/[\\/]/).some((part) => part === 'tests' || part === 'benches')) return true;
  const lines = source.split('\n').slice(0, line - 1);
  return lines.some((text, i) => {
    if (!text.startsWith('#[cfg(test)]')) return false;
    const rest = text.slice('#[cfg(test)]'.length).trim() || (lines[i + 1] ?? '').trim();
    return /^(pub(\([a-z]+\))? )?mod\b/.test(rest);
  });
}

// Absolute file path -> line ranges that differ from the branch point with `base`.
function changedLines(base) {
  const repoRoot = run('git', ['rev-parse', '--show-toplevel']).trim();
  const git = (args) => run('git', ['-c', 'core.quotePath=false', ...args], { cwd: repoRoot });
  const point = base === 'HEAD' ? 'HEAD' : git(['merge-base', base, 'HEAD']).trim();
  const diff = git(['diff', '--unified=0', '--no-color', '--no-ext-diff', '--src-prefix=a/', '--dst-prefix=b/', point, '--', '*.rs']);
  const ranges = new Map();
  for (const [file, list] of parseDiff(diff)) ranges.set(path.resolve(repoRoot, file), list);
  const untracked = git(['ls-files', '-z', '--others', '--exclude-standard', '--', '*.rs']);
  for (const name of untracked.split('\0').filter(Boolean)) ranges.set(path.resolve(repoRoot, name), [[1, Infinity]]);
  return ranges;
}

function hasCommits() {
  return spawnSync('git', ['rev-parse', '--verify', '--quiet', 'HEAD'], { stdio: 'ignore' }).status === 0;
}

async function clippy(cargoArgs) {
  const flags = Object.keys(LINTS).flatMap((lint) => ['-W', lint]);
  const child = spawn('cargo', ['clippy', '--message-format=json', ...cargoArgs, '--', ...flags], {
    stdio: ['ignore', 'pipe', 'inherit'],
  });
  const messages = [];
  let finished = null;
  for await (const line of createInterface({ input: child.stdout })) {
    if (!line.startsWith('{')) continue;
    let parsed;
    try {
      parsed = JSON.parse(line);
    } catch {
      continue;
    }
    if (parsed.reason === 'compiler-message') messages.push(parsed.message);
    if (parsed.reason === 'build-finished') finished = parsed.success;
  }
  await new Promise((resolve) => child.on('close', resolve));
  if (finished !== true) {
    for (const message of messages.filter((m) => m.level === 'error')) console.error(message.rendered ?? message.message);
    fail('cargo clippy did not finish; fix the errors above first');
  }
  return messages;
}

// Every line range a finding points at: its primary span, plus the spans of
// its notes ("first possible panic found here"), so a change in a function
// body that causes a finding on the signature still counts as yours.
function findingRanges(message) {
  const spans = [...message.spans.filter((s) => s.is_primary), ...message.children.flatMap((c) => c.spans)];
  return spans.map((s) => ({ file: s.file_name, first: s.line_start, last: s.line_end }));
}

async function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    fail(`${error.message}\n${USAGE}`);
  }
  if (options.help) {
    console.log(USAGE);
    return 0;
  }

  const workspaceRoot = path.dirname(run('cargo', ['locate-project', '--workspace', '--message-format', 'plain']).trim());
  const everything = options.all || (options.base === 'HEAD' && !hasCommits());
  const ranges = everything ? null : changedLines(options.base);
  const messages = await clippy(options.cargoArgs);

  const seen = new Set();
  const findings = [];
  for (const message of messages) {
    const code = message.code?.code;
    const lint = CODE_ALIASES[code] ?? code;
    if (!lint || !(lint in LINTS)) continue;
    const span = message.spans.find((candidate) => candidate.is_primary);
    if (!span) continue;
    const file = path.resolve(workspaceRoot, span.file_name);
    if (ranges && !findingRanges(message).some((r) => touches(ranges, path.resolve(workspaceRoot, r.file), r.first, r.last))) continue;
    if (PRODUCTION_ONLY_LINTS.has(lint)) {
      let source = '';
      try {
        source = readFileSync(file, 'utf8');
      } catch {
        // An unreadable file is treated as production code.
      }
      if (isTestCode(path.relative(workspaceRoot, file), span.line_start, source)) continue;
    }
    const key = `${lint}:${file}:${span.line_start}`;
    if (seen.has(key)) continue;
    seen.add(key);
    findings.push({ lint, file: path.relative(process.cwd(), file), line: span.line_start, text: message.message });
  }

  findings.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line);
  for (const finding of findings) {
    console.log(`${finding.file}:${finding.line}  ${finding.lint}`);
    console.log(`    ${finding.text}`);
    console.log(`    rule: ${LINTS[finding.lint]}`);
  }
  const scope = everything ? 'in the code clippy compiled' : `on lines changed since ${options.base}`;
  console.log(findings.length === 0 ? `book-lints: no findings ${scope}` : `book-lints: ${findings.length} finding(s) ${scope}`);
  return findings.length === 0 ? 0 : 1;
}

// Run only as a script, so the parsing functions can be imported on their own.
if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))) {
  process.exitCode = await main();
}
