// Does the orphan-test guard do what it promises: each rule reports a test CI
// never runs, and leaves alone one that CI does run. Every test builds a small
// fake repository, so each case shows exactly which line of CI makes the
// difference.
//
//   node --test script/orphan-tests.test.mjs

import { strict as assert } from 'node:assert';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { findOrphans, globToRegExp, workflowFragments } from './orphan-tests.mjs';

/** Writes `files` into a fresh directory and runs the guard over them. */
function check(files, options = {}) {
  const root = mkdtempSync(join(tmpdir(), 'orphan-tests-'));
  try {
    for (const [path, body] of Object.entries(files)) {
      mkdirSync(dirname(join(root, path)), { recursive: true });
      writeFileSync(join(root, path), body, { mode: path.endsWith('.sh') ? 0o755 : 0o644 });
    }
    return findOrphans(root, Object.keys(files), { runners: [], exceptions: [], ...options });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

const files = (findings, rule) => findings.filter((f) => f.rule === rule).map((f) => f.file).sort();

/** A workflow with one job whose steps run `commands`. */
function workflow(...steps) {
  const body = steps
    .map((s) => {
      const lines = [`      - name: step`];
      if (s.wd) lines.push(`        working-directory: ${s.wd}`);
      if (s.env) {
        lines.push('        env:');
        for (const [k, v] of Object.entries(s.env)) lines.push(`          ${k}: ${v}`);
      }
      lines.push('        run: |', ...s.run.split('\n').map((l) => `          ${l}`));
      return lines.join('\n');
    })
    .join('\n');
  return `name: CI\non: push\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n${body}\n`;
}

const PKG = (scripts) => JSON.stringify({ name: 'x', scripts });
const VITEST = `import { defineConfig } from 'vitest/config';
export default defineConfig({
  // a comment with a /* that is not one
  test: { include: ['src/**/*.test.{js,ts}'], exclude: ['src/slow/**'] },
});\n`;

// --- Rule 1: JS tests ----------------------------------------------------

test('js: a test vitest picks up in a package CI tests passes', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'npm test -w app' }),
    'app/package.json': PKG({ test: 'vitest run' }),
    'app/vitest.config.ts': VITEST,
    'app/src/a.test.ts': '',
  });
  assert.deepEqual(files(findings, 'js'), []);
});

test('js: a test in a package CI never tests is reported', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'npm test -w other' }),
    'other/package.json': PKG({ test: 'vitest run' }),
    'app/package.json': PKG({ test: 'vitest run' }),
    'app/vitest.config.ts': VITEST,
    'app/src/a.test.ts': '',
  });
  assert.deepEqual(files(findings, 'js'), ['app/src/a.test.ts']);
});

test('js: a test outside the vitest include, or inside its exclude, is reported', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'npm test -w app' }),
    'app/package.json': PKG({ test: 'vitest run' }),
    'app/vitest.config.ts': VITEST,
    'app/src/a.test.ts': '',
    'app/tests/b.test.ts': '',
    'app/src/slow/c.test.ts': '',
  });
  assert.deepEqual(files(findings, 'js'), ['app/src/slow/c.test.ts', 'app/tests/b.test.ts']);
});

test('js: working-directory plus npm test, and vitest --root, count as runs', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow(
      { run: 'npm ci\nnpm test', wd: 'ui' },
      { run: 'npx vitest run --root server' },
    ),
    'ui/package.json': PKG({ test: 'vitest run' }),
    'ui/src/a.test.js': '',
    'server/b.test.js': '',
    'lost/c.test.js': '',
  });
  assert.deepEqual(files(findings, 'js'), ['lost/c.test.js']);
});

test('js: node --test through a recipe a hook calls passes; through a recipe nobody calls it is reported', () => {
  const findings = check({
    '.pre-commit-config.yaml': `repos:
  - repo: local
    hooks:
      - id: a
        entry: just a-test
        language: system
`,
    Justfile: 'a-test:\n    node --test script/a.test.mjs\n\nb-test:\n    node --test script/b.test.mjs\n',
    'script/a.test.mjs': '',
    'script/b.test.mjs': '',
  });
  assert.deepEqual(files(findings, 'js'), ['script/b.test.mjs']);
});

test('js: an npm script that names the test, reached through a chain, counts', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'npm run check', wd: 'docs' }),
    'docs/package.json': PKG({ check: 'npm run prose', prose: 'node --test ../tools/p.test.mjs' }),
    'tools/p.test.mjs': '',
  });
  assert.deepEqual(files(findings, 'js'), []);
});

test('js: a vitest include with a character class, and a coverage include before it, read right', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'npm test -w app' }),
    'app/package.json': PKG({ test: 'vitest run' }),
    'app/vitest.config.js': `export default {
  test: {
    coverage: { include: ['lib/**'] },
    include: ['src/**/*.test.[jt]s'],
  },
};\n`,
    'app/src/a.test.ts': '',
    'app/lib/b.test.ts': '',
  });
  assert.deepEqual(files(findings, 'js'), ['app/lib/b.test.ts']);
});

test('js: node --test over a glob or a directory counts for the files in it', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'node --test script/*.test.mjs\nnode --test tools/' }),
    'script/a.test.mjs': '',
    'tools/sub/b.test.mjs': '',
    'other/c.test.mjs': '',
  });
  assert.deepEqual(files(findings, 'js'), ['other/c.test.mjs']);
});

test('js: an npm pre-script counts as part of the script', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'npm test -w app' }),
    'app/package.json': PKG({ pretest: 'node --test scripts/a.test.mjs', test: 'echo' }),
    'app/scripts/a.test.mjs': '',
  });
  assert.deepEqual(files(findings, 'js'), []);
});

// --- Rule 2: shell tests ---------------------------------------------------

test('sh: a hook entry or a workflow step that runs the script passes', () => {
  const findings = check({
    '.pre-commit-config.yaml': `repos:
  - repo: local
    hooks:
      - id: a
        entry: script/a.test.sh
        language: system
        files: ^script/a
`,
    '.github/workflows/ci.yml': workflow({ run: 'bash ../script/b.test.sh', wd: 'docs' }),
    'docs/.keep': '',
    'script/a.test.sh': '',
    'script/b.test.sh': '',
  });
  assert.deepEqual(files(findings, 'sh'), []);
});

test('sh: a script only an uncalled recipe runs is reported', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'just used' }),
    Justfile: 'used:\n    echo hi\n\nunused:\n    script/a.test.sh\n',
    'script/a.test.sh': '',
  });
  assert.deepEqual(files(findings, 'sh'), ['script/a.test.sh']);
});

test('sh: a recipe dependency of a called recipe counts', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'just check' }),
    Justfile: 'check: lint (inner "x")\n    echo\n\nlint:\n    echo\n\ninner arg:\n    script/a.test.sh\n',
    'script/a.test.sh': '',
  });
  assert.deepEqual(files(findings, 'sh'), []);
});

test('sh: a hook that never runs in CI (manual stage, files matching nothing) does not count', () => {
  const findings = check({
    '.pre-commit-config.yaml': `repos:
  - repo: local
    hooks:
      - id: a
        entry: script/a.test.sh
        language: system
        stages: [manual]
      - id: b
        entry: script/b.test.sh
        language: system
        files: ^nowhere/
`,
    'script/a.test.sh': '',
    'script/b.test.sh': '',
  });
  assert.deepEqual(files(findings, 'sh'), ['script/a.test.sh', 'script/b.test.sh']);
});

test('sh: a comment that names the script is no run', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: '# script/a.test.sh used to run here\necho' }),
    'script/a.test.sh': '',
  });
  assert.deepEqual(files(findings, 'sh'), ['script/a.test.sh']);
});

test('sh: echo or git naming a script does not run it; if and ! in front still do', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({
      run: 'echo "skipping script/a.test.sh"\ngit diff -- script/b.test.sh\nif script/c.test.sh; then echo ok; fi\n! bash script/d.test.sh',
    }),
    'script/a.test.sh': '',
    'script/b.test.sh': '',
    'script/c.test.sh': '',
    'script/d.test.sh': '',
  });
  assert.deepEqual(files(findings, 'sh'), ['script/a.test.sh', 'script/b.test.sh']);
});

test('sh: a step with if: false never runs', () => {
  const ci = `name: CI
on: push
jobs:
  t:
    steps:
      - name: off
        if: false
        run: script/a.test.sh
      - name: on
        if: github.event_name == 'push'
        run: script/b.test.sh
`;
  const findings = check({ '.github/workflows/ci.yml': ci, 'script/a.test.sh': '', 'script/b.test.sh': '' });
  assert.deepEqual(files(findings, 'sh'), ['script/a.test.sh']);
});

test('sh: a script CI runs counts with what it runs in turn', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'script/all.sh' }),
    'script/all.sh': '#!/usr/bin/env bash\nbash "$(dirname "$0")/a.test.sh"\n',
    'script/a.test.sh': '',
  });
  assert.deepEqual(files(findings, 'sh'), []);
});

test('sh: a cd in one line of a plain recipe does not carry to the next', () => {
  const findings = check({
    '.github/workflows/ci.yml': workflow({ run: 'just r s' }),
    Justfile: 'r:\n    cd docs\n    script/a.test.sh\n\ns:\n    #!/usr/bin/env bash\n    cd docs\n    script/b.test.sh\n',
    'script/a.test.sh': '',
    'script/b.test.sh': '',
    'docs/script/b.test.sh': '',
  });
  // In the shebang recipe the cd holds, so it runs docs/script/b.test.sh.
  assert.deepEqual(files(findings, 'sh'), ['script/b.test.sh']);
});

test('sh: a matrix value fills in the recipe a step calls', () => {
  const ci = `name: CI
on: push
jobs:
  t:
    strategy:
      matrix:
        include:
          - recipe: a
          - recipe: b
    steps:
      - run: just \${{ matrix.recipe }}
`;
  assert.deepEqual(workflowFragments(ci, 'ci.yml').map((f) => f.text), ['just a', 'just b']);
  const findings = check({
    '.github/workflows/ci.yml': ci,
    Justfile: 'a:\n    script/a.test.sh\n\nb:\n    script/b.test.sh\n',
    'script/a.test.sh': '',
    'script/b.test.sh': '',
  });
  assert.deepEqual(files(findings, 'sh'), []);
});

test('sh: a leg listed under matrix exclude does not run', () => {
  const ci = `name: CI
on: push
jobs:
  t:
    strategy:
      matrix:
        recipe: [a, b]
        exclude:
          - recipe: c
    steps:
      - run: just \${{ matrix.recipe }}
`;
  assert.deepEqual(workflowFragments(ci, 'ci.yml').map((f) => f.text), ['just a', 'just b']);
});

// --- Crates ------------------------------------------------------------------

const WORKSPACE = {
  'packages/Cargo.toml': '[workspace]\nmembers = [\n  "a",\n  "b",\n]\n',
  'packages/a/Cargo.toml': '[package]\nname = "crate-a"\n',
  'packages/b/Cargo.toml': '[package]\nname = "crate-b"\n',
};

test('crate: a workspace test with an exclude, and that crate under -p elsewhere, covers both', () => {
  const findings = check({
    ...WORKSPACE,
    '.github/workflows/ci.yml': workflow({ run: 'cd packages\ncargo test --workspace --exclude crate-b\ncargo test -p crate-b' }),
  });
  assert.deepEqual(files(findings, 'crate'), []);
});

test('crate: an excluded crate no other run tests is reported', () => {
  const findings = check({
    ...WORKSPACE,
    Justfile: 'excl := "--exclude crate-b"\n\ntest:\n    cd packages && cargo test --workspace {{excl}}\n',
    '.github/workflows/ci.yml': workflow({ run: 'just test' }),
  });
  assert.deepEqual(files(findings, 'crate'), ['packages/b/Cargo.toml']);
});

test('crate: a run of one target does not count as testing the crate', () => {
  const findings = check({
    ...WORKSPACE,
    '.github/workflows/ci.yml': workflow({ run: 'cargo test -p crate-a\ncargo test --test only', wd: 'packages/b' }),
  });
  assert.deepEqual(files(findings, 'crate'), ['packages/b/Cargo.toml']);
});

test('crate: --exclude=name excludes too', () => {
  const findings = check({
    ...WORKSPACE,
    '.github/workflows/ci.yml': workflow({ run: 'cargo test --workspace --exclude=crate-b', wd: 'packages' }),
  });
  assert.deepEqual(files(findings, 'crate'), ['packages/b/Cargo.toml']);
});

// --- Rule 3: feature files ---------------------------------------------------

const ENGINE = {
  'packages/Cargo.toml': '[workspace]\nmembers = ["engine", "napp"]\n',
  'packages/engine/Cargo.toml': '[package]\nname = "regelrecht-engine"\n\n[[test]]\nname = "bdd"\nharness = false\ntest = false\n',
  'packages/napp/Cargo.toml': '[package]\nname = "napp"\n\n[[test]]\nname = "bdd"\nharness = false\n',
  'packages/napp/tests/bdd/main.rs': '',
  'bdd/conformance/a.feature': '',
  'corpus/demo/law/scenarios/b.feature': '',
  'corpus/demo/law/notes.feature': '',
  'corpus/regulation/law/scenarios/c.feature': '',
  'corpus-poc/napp/scenarios/d.feature': '',
};

test('feature: the engine buckets CI runs cover their own directories only', () => {
  const findings = check({
    ...ENGINE,
    '.github/workflows/ci.yml': workflow(
      { run: 'cargo test --test bdd', wd: 'packages/engine', env: { BDD_BUCKET: 'conformance' } },
      {
        run: 'cargo test --test bdd',
        wd: 'packages/engine',
        env: { BDD_BUCKET: 'corpus', REGULATION_PATH: '${{ github.workspace }}/corpus/demo' },
      },
    ),
  });
  // notes.feature sits under no scenarios/ directory, which bucket A requires.
  assert.deepEqual(files(findings, 'feature'), [
    'corpus-poc/napp/scenarios/d.feature',
    'corpus/demo/law/notes.feature',
    'corpus/regulation/law/scenarios/c.feature',
  ]);
});

test('feature: without a conformance run, bdd/conformance is reported', () => {
  const findings = check({
    ...ENGINE,
    '.github/workflows/ci.yml': workflow({
      run: 'cargo test --test bdd',
      wd: 'packages/engine',
      env: { BDD_BUCKET: 'corpus', REGULATION_PATH: '${{ github.workspace }}/corpus/demo' },
    }),
  });
  assert.ok(files(findings, 'feature').includes('bdd/conformance/a.feature'));
});

test('feature: without BDD_BUCKET the engine runs both buckets over the default corpus', () => {
  const findings = check({
    ...ENGINE,
    '.github/workflows/ci.yml': workflow({ run: 'if cargo test --test bdd; then echo; fi', wd: 'packages/engine' }),
  });
  assert.deepEqual(files(findings, 'feature'), [
    'corpus-poc/napp/scenarios/d.feature',
    'corpus/demo/law/notes.feature',
    'corpus/demo/law/scenarios/b.feature',
  ]);
});

test('feature: the bucket comes from export, inline assignment and workflow-level env alike', () => {
  const only = (run, env) => {
    const ci = workflow({ run, wd: 'packages/engine' });
    const text = env ? ci.replace('on: push\n', `on: push\nenv:\n  BDD_BUCKET: ${env}\n`) : ci;
    return files(check({ ...ENGINE, '.github/workflows/ci.yml': text }), 'feature');
  };
  const conformanceOnly = [
    'corpus-poc/napp/scenarios/d.feature',
    'corpus/demo/law/notes.feature',
    'corpus/demo/law/scenarios/b.feature',
    'corpus/regulation/law/scenarios/c.feature',
  ];
  assert.deepEqual(only('export BDD_BUCKET=conformance\ncargo test --test bdd'), conformanceOnly);
  assert.deepEqual(only('BDD_BUCKET=conformance cargo test --test bdd'), conformanceOnly);
  assert.deepEqual(only('cargo test --test bdd', 'conformance'), conformanceOnly);
});

test('feature: REGULATION_PATH built from a script in the step is followed', () => {
  const findings = check({
    ...ENGINE,
    'script/dirs.sh': '#!/usr/bin/env bash\necho corpus/demo\necho corpus/regulation\n',
    '.github/workflows/ci.yml': workflow({
      run: 'DIRS=$(../../script/dirs.sh)\nREGULATION_PATH=$(echo "$DIRS" | sed "s|^|$GITHUB_WORKSPACE/|" | paste -sd:)\nexport REGULATION_PATH\ncargo test --test bdd',
      wd: 'packages/engine',
      env: { BDD_BUCKET: 'corpus' },
    }),
  });
  assert.deepEqual(files(findings, 'feature'), [
    'bdd/conformance/a.feature',
    'corpus-poc/napp/scenarios/d.feature',
    'corpus/demo/law/notes.feature',
  ]);
});

test('feature: a declared runner covers its directory only when the runner runs in CI', () => {
  const runners = [{ runner: 'packages/napp/tests/bdd/main.rs', root: 'corpus-poc/napp/scenarios' }];
  const tested = check(
    { ...ENGINE, '.github/workflows/ci.yml': workflow({ run: 'cargo test -p napp', wd: 'packages' }) },
    { runners },
  );
  assert.ok(!files(tested, 'feature').includes('corpus-poc/napp/scenarios/d.feature'));
  const untested = check(
    { ...ENGINE, '.github/workflows/ci.yml': workflow({ run: 'echo', wd: 'packages' }) },
    { runners },
  );
  assert.ok(files(untested, 'feature').includes('corpus-poc/napp/scenarios/d.feature'));
});

test('feature: a runner target with test = false needs a run that names it', () => {
  const runners = [{ runner: 'packages/napp/tests/bdd/main.rs', root: 'corpus-poc/napp/scenarios' }];
  const manifest = '[package]\nname = "napp"\n\n[[test]]\nname = "bdd"\nharness = false\ntest = false\n';
  const workspaceOnly = check(
    { ...ENGINE, 'packages/napp/Cargo.toml': manifest, '.github/workflows/ci.yml': workflow({ run: 'cargo test -p napp' }) },
    { runners },
  );
  assert.ok(files(workspaceOnly, 'feature').includes('corpus-poc/napp/scenarios/d.feature'));
  const named = check(
    { ...ENGINE, 'packages/napp/Cargo.toml': manifest, '.github/workflows/ci.yml': workflow({ run: 'cargo test -p napp --test bdd' }) },
    { runners },
  );
  assert.ok(!files(named, 'feature').includes('corpus-poc/napp/scenarios/d.feature'));
});

test('exception: a listed path is left alone, and a stale exception is a finding', () => {
  const exceptions = [
    { rule: 'feature', path: 'corpus/regulation/', reason: 'on purpose' },
    { rule: 'sh', path: 'script/gone.test.sh', reason: 'stale' },
  ];
  const findings = check({ 'corpus/regulation/law/scenarios/c.feature': '' }, { exceptions });
  assert.deepEqual(files(findings, 'feature'), []);
  assert.deepEqual(files(findings, 'exception'), ['script/gone.test.sh']);
});

// --- Rule 4: skips without a reason ------------------------------------------

test('wip: a reason above (past other tags) or on the same line passes; a bare @wip is reported', () => {
  const findings = check({
    'a.feature': `Feature: x
  # The engine cannot do this yet.
  @slow
  @wip
  Scenario: one

  @wip # waiting for article 8
  Scenario: two

  Given something
  @wip
  Scenario: three

  # @wip in a comment is no tag
  Scenario: four
`,
  });
  assert.deepEqual(
    findings.filter((f) => f.rule === 'wip').map((f) => f.line),
    [11],
  );
});

test('ignore: a reason as value, trailing comment or comment above passes; a bare #[ignore] is reported', () => {
  const findings = check({
    'a.rs': `#[test]
#[ignore = "needs network"]
fn one() {}

#[test]
#[ignore] // needs network
fn two() {}

/// Needs network access.
#[tokio::test]
#[ignore]
fn three() {}

fn helper() {}
#[test]
#[ignore]
fn four() {}

#[ignore = ""]
fn five() {}
`,
  });
  assert.deepEqual(
    findings.filter((f) => f.rule === 'ignore').map((f) => f.line),
    [16, 19],
  );
});

// --- Glob ----------------------------------------------------------------

test('glob: vitest default include and brace lists match like vitest', () => {
  const def = globToRegExp('**/*.{test,spec}.?(c|m)[jt]s?(x)');
  for (const p of ['a.test.ts', 'src/a/b.spec.mjs', 'x.test.jsx']) assert.ok(def.test(p), p);
  for (const p of ['a.ts', 'a.test.sh', 'a.test.py']) assert.ok(!def.test(p), p);
  const src = globToRegExp('src/**/*.test.{js,ts}');
  assert.ok(src.test('src/a.test.ts'));
  assert.ok(src.test('src/x/y/a.test.js'));
  assert.ok(!src.test('tests/a.test.ts'));
});
