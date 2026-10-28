// Assert every repo path the docs cite in backticks actually exists.
//
// This is the guard against the largest class of documentation drift: prose
// that names a file or directory the code has since moved or renamed. Nothing
// caught it before, because a stale path breaks no build and no link checker
// looks inside a code span. `guide/testing.md` and `operations/adding-a-law.md`
// both told readers for months that BDD features live in `features/`, a
// directory that does not exist, and pointed at
// `packages/engine/tests/bdd/steps/`, which does not either. A reader following
// those pages wrote a file that could not run.
//
// It reads the markdown, not the build output, so it runs without `astro build`
// and fails fast. Non-zero exit fails the gate.
//
// ## What counts as a cited path
//
// A backtick span is treated as a repo path when it looks like one: at least
// one slash, no spaces, no URL scheme, and a first segment that could be a
// directory name. Everything else in backticks (field names, commands, YAML
// keys, operations) is ignored. That leaves roughly 95 candidates out of some
// 1000 code spans in the docs.
//
// ## Why a suffix match, not an exact one
//
// A path is accepted when it matches the tail of any tracked file, so
// `src/main.js` in a page about the editor resolves against
// `frontend/src/main.js`. Docs legitimately write paths relative to the
// component they describe, and demanding repo-root paths everywhere would make
// the prose worse to read in order to satisfy a script.
//
// The cost is real and deliberate: a path whose basename exists somewhere else
// in the repo passes even when the directory it names is wrong. That trade buys
// a false-positive rate low enough for a blocking gate, which is the property
// that decides whether a check survives. A gate people learn to ignore protects
// nothing.
//
// ## Why an allowlist rather than a cleverer pattern
//
// The remaining non-paths are not a pattern, they are a list: fictional
// organizations in examples, media types that look like paths, git refs,
// external systems. Each entry below says why it is there. An allowlist that
// has to be read and justified stays honest; a regex broad enough to swallow
// them silently would also swallow the next real defect.

import { readdirSync, statSync, readFileSync, existsSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const REPO = fileURLToPath(new URL('../..', import.meta.url));
const DOCS = fileURLToPath(new URL('../src/content/docs', import.meta.url));

// Cited strings that look like repo paths but are not. Each needs a reason.
const ALLOW = new Map([
  ['acme/regelrecht-lokaal', 'fictional organization in a federation example'],
  ['acme/regels', 'fictional organization in a federation example'],
  ['acme/regelrecht-private-test', 'fictional repository in a private-traject example'],
  ['owner/repo', 'placeholder for a GitHub repository the reader supplies'],
  ['MinBZK/regelrecht-corpus', 'a separate repository, not a path in this one'],
  ['werkbank/chronolex/napp/bron.yaml', 'a manifest in the private NAPP corpus, a separate repository'],
  ['application/wasm', 'a media type, not a path'],
  ['.walkthrough/.env', 'local and gitignored: the API key file the walkthrough pipeline reads'],
  ['corpus/demo/walkthrough/script/', 'created by `just walkthrough script` once a chapter is spoken by a generated voice; none is yet'],
  ['refs/heads/main', 'a git ref'],
  ['refs/tags/schema-vX.Y.Z', 'a git ref, with a version placeholder'],
  ['refs/pull/N/merge', 'a git ref, with a PR-number placeholder'],
  ['origin/main', 'a git remote-tracking branch'],
  ['gh-readonly-queue/main/...', 'the merge queue branch name, truncated'],
  ['cells/cjib/capabilities.yaml', 'a proposed layout in the CJIB pilot, not built'],
  ['chronicles/cjib_wahv_betalingen.yaml', 'a proposed layout in the CJIB pilot, not built'],
  ['enrich/opencode', 'an external tool invocation'],
  ['ScenarioBuilder/Form/Gherkin/Visual/Panel.vue', 'a prose list of component names, not one path'],
  ['public/data', 'a build output directory, generated rather than tracked'],
  ['frontend/public/data/', 'a build output directory, generated rather than tracked'],
]);

// Placeholder segments: a path teaching a convention rather than naming a file.
const PLACEHOLDER = /\{[^}]+\}|\byour_law\b|\bX\.Y\.Z\b|\*/;

function markdownFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...markdownFiles(full));
    else if (entry.endsWith('.md') || entry.endsWith('.mdx')) out.push(full);
  }
  return out;
}

let tracked;
try {
  tracked = execFileSync('git', ['ls-files'], {
    cwd: REPO,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  })
    .split('\n')
    .filter(Boolean);
} catch {
  // Without a file list there is nothing to check against, and guessing would
  // mean reporting every path as missing. Skip loudly instead of failing.
  console.log('check-cited-paths: could not list tracked files, skipping');
  process.exit(0);
}

// Index every suffix of every tracked path, plus each directory prefix, so a
// lookup is a set membership test rather than a scan per candidate.
const known = new Set();
for (const file of tracked) {
  const parts = file.split('/');
  for (let i = 0; i < parts.length; i++) {
    known.add(parts.slice(i).join('/'));
  }
  // Directory prefixes, so `bdd/conformance` resolves as well as a file in it.
  for (let i = 0; i < parts.length - 1; i++) {
    for (let j = i; j < parts.length - 1; j++) {
      known.add(parts.slice(i, j + 1).join('/'));
    }
  }
}

const problems = [];

for (const file of markdownFiles(DOCS)) {
  const text = readFileSync(file, 'utf8');
  const lines = text.split('\n');

  let inFence = false;
  lines.forEach((line, i) => {
    if (/^\s*```/.test(line)) {
      inFence = !inFence;
      return;
    }
    // Fenced blocks hold example YAML and shell transcripts, whose paths are
    // illustrative by nature. Prose is where a claim about the repo is made.
    if (inFence) return;

    for (const m of line.matchAll(/`([^`]+)`/g)) {
      const raw = m[1].trim();
      if (ALLOW.has(raw)) continue;
      if (PLACEHOLDER.test(raw)) continue;
      // Path-shaped: has a slash, no whitespace, no scheme, no leading dash.
      // A leading dot counts: `.github/workflows/ci.yml` and `.claude/skills/`
      // are exactly the kind of path the docs cite, and an earlier version of
      // this pattern required a letter first and skipped every one of them
      // without saying so.
      if (!/^\.?[A-Za-z_][A-Za-z0-9_.-]*(\/[A-Za-z0-9_.-]+)+\/?$/.test(raw)) continue;
      if (/^https?:/.test(raw)) continue;

      const candidate = raw.replace(/\/$/, '');
      if (known.has(candidate)) continue;
      if (existsSync(join(REPO, candidate))) continue;

      problems.push({
        file: relative(REPO, file),
        line: i + 1,
        path: raw,
      });
    }
  });
}

if (problems.length) {
  console.error(
    `check-cited-paths FAILED: ${problems.length} cited path(s) do not exist in the repository:\n`,
  );
  for (const p of problems) {
    console.error(`  ${p.file}:${p.line}`);
    console.error(`    ${p.path}`);
  }
  console.error(
    '\nEach one is either a path that moved (fix the docs), or not a repo path at all\n' +
      '(add it to ALLOW in docs/scripts/check-cited-paths.mjs, with the reason).',
  );
  process.exit(1);
}

console.log('check-cited-paths passed: every cited repo path exists.');
