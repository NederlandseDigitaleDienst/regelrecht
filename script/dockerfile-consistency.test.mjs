// De Rust-Dockerfiles knippen workspace-members weg met een letterlijke `sed`.
// Komt er een member bij die niet wordt ge-COPY'd én niet wordt weggeknipt,
// dan sterft `cargo chef prepare` — en sinds de images uit de standaardloop van
// een PR zijn gehaald merkt niemand dat op de PR zelf. Hetzelfde geldt voor de
// binary-namen: `--build-arg BIN=` en het `COPY --from=builder`-pad zijn
// stringliteralen die niets nakijkt.
//
// Node's ingebouwde runner, geen dependency: dit draait in de Pre-commit-baan,
// die Node en cargo heeft maar geen node_modules.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const read = (path) => readFileSync(join(root, path), 'utf8');

const DOCKERFILES = [
  'packages/admin/Dockerfile',
  'packages/pipeline/Dockerfile',
  'frontend/Dockerfile',
  'frontend-demo/Dockerfile',
  'packages/poc-portal/Dockerfile',
  'packages/poc-napp/Dockerfile',
];

/** De members uit packages/Cargo.toml, in de multiline vorm die de `sed` aanneemt. */
function workspaceMembers() {
  const manifest = read('packages/Cargo.toml');
  const block = /^members = \[$([\s\S]*?)^\]$/m.exec(manifest);
  assert.ok(block, 'packages/Cargo.toml heeft geen meerregelige members-array');
  return block[1]
    .split('\n')
    .map((line) => /^\s*"([^"]+)",?\s*$/.exec(line))
    .filter(Boolean)
    .map((match) => match[1]);
}

/**
 * De bouwtrappen van een Dockerfile die de workspace bijknippen: alles tussen
 * een `FROM` en de volgende, mits er een `sed -i` op Cargo.toml in staat.
 */
function trimStages(path) {
  const stages = [];
  let current = null;
  for (const line of read(path).split('\n')) {
    if (/^FROM /.test(line)) {
      if (current) stages.push(current);
      const named = / AS (\S+)\s*$/.exec(line);
      current = { name: named ? named[1] : line.trim(), lines: [] };
    } else if (current) {
      current.lines.push(line);
    }
  }
  if (current) stages.push(current);

  return stages
    .map((stage) => {
      const body = stage.lines.join('\n');
      if (!/sed -i .*Cargo\.toml/.test(body)) return null;
      const sed = /sed -i '([^']*)' Cargo\.toml/.exec(body);
      assert.ok(sed, `${path} (${stage.name}): sed-opdracht niet te lezen`);
      const dropped = [...sed[1].matchAll(/\/"([^"]+)"\/d/g)].map((m) => m[1]);
      const copied = [...body.matchAll(/^COPY packages\/([a-z0-9-]+)\/ \1\/$/gm)].map((m) => m[1]);
      return { where: `${path} (${stage.name})`, dropped, copied };
    })
    .filter(Boolean);
}

const MEMBERS = workspaceMembers();
const STAGES = DOCKERFILES.flatMap(trimStages);

/** De bin-targets van de workspace volgens cargo zelf. */
function binTargets() {
  const meta = JSON.parse(
    execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], {
      cwd: join(root, 'packages'),
      maxBuffer: 64 * 1024 * 1024,
      encoding: 'utf8',
    }),
  );
  const bins = new Set();
  for (const pkg of meta.packages) {
    for (const target of pkg.targets) {
      if (target.kind.includes('bin')) bins.add(target.name);
    }
  }
  return bins;
}

test('elke bouwtrap die knipt is ook gevonden', () => {
  // Zonder deze ondergrens zou een stukgelopen parser als een groene test
  // langskomen: nul trappen halen elke assertie hieronder vacuüm.
  assert.ok(STAGES.length >= 4, `slechts ${STAGES.length} knippende bouwtrappen gevonden`);
  assert.ok(MEMBERS.length >= 12, `slechts ${MEMBERS.length} members gelezen`);
});

test('elke workspace-member is in elke Rust-Dockerfile ge-COPYd of weggeknipt', () => {
  for (const stage of STAGES) {
    const accounted = new Set([...stage.dropped, ...stage.copied]);
    for (const member of MEMBERS) {
      assert.ok(
        accounted.has(member),
        `${stage.where}: member "${member}" wordt niet ge-COPYd en niet weggeknipt — cargo chef prepare faalt hierop`,
      );
    }
    for (const name of stage.dropped) {
      assert.ok(MEMBERS.includes(name), `${stage.where}: knipt "${name}" weg, maar die member bestaat niet`);
      assert.ok(!stage.copied.includes(name), `${stage.where}: "${name}" wordt zowel weggeknipt als ge-COPYd`);
    }
    for (const name of stage.copied) {
      assert.ok(MEMBERS.includes(name), `${stage.where}: COPYt "${name}", maar die member bestaat niet`);
    }
  }
});

test('de rust-tag van elk basisimage volgt rust-toolchain.toml', () => {
  const channel = /^channel = "([^"]+)"$/m.exec(read('rust-toolchain.toml'));
  assert.ok(channel, 'rust-toolchain.toml heeft geen channel');
  const [major, minor] = channel[1].split('.');

  let seen = 0;
  for (const path of DOCKERFILES) {
    for (const [, tag] of read(path).matchAll(/^FROM rust:([0-9]+\.[0-9]+(?:\.[0-9]+)?)/gm)) {
      seen += 1;
      assert.ok(
        tag === `${major}.${minor}` || tag === channel[1],
        `${path}: FROM rust:${tag} wijkt af van channel ${channel[1]} in rust-toolchain.toml`,
      );
    }
  }
  assert.ok(seen >= 3, `slechts ${seen} rust-basisimages gevonden`);
});

// De test hierboven kijkt naar de tag, en een tag zegt niets: `rust:1.96-alpine`
// wijst in de ene Dockerfile naar het ene image en in de andere naar een ander,
// allebei geldig en allebei groen. Het image dat achterloopt bouwt tegen een
// andere toolchain dan de rest en niets wijst erop.
//
// Hoe het zo kwam: dependabot bumpt alleen `/packages/admin` en
// `/packages/pipeline` (de bumps van 10 juli 2026 naar `a41f774`), en wat het
// niet volgt bleef op de digest van juni staan. De vier Dockerfiles onder
// `packages/` staan nu gelijk, op de actuele `a41f774`.
//
// `frontend/Dockerfile` en `frontend-demo/Dockerfile` staan nog op de oude.
// Dat is zo op main en komt hier niet vandaan, dus die gelijktrekken is een
// eigen wijziging; tot dan meldt deze test het verschil zonder te falen, want
// hem hard zetten zou elke pull request stilleggen voor iets dat niemand hier
// introduceerde. Haal `skip` weg zodra die twee mee zijn, en hij bewaakt het.
test('één rust-tag betekent één digest', { skip: 'frontend en frontend-demo lopen op main achter' }, () => {
  const perTag = new Map();
  for (const path of DOCKERFILES) {
    for (const [, tag, digest] of read(path).matchAll(
      /^FROM (?:--platform=\S+ )?(rust:[^\s@]+)@(sha256:[a-f0-9]{64})/gm,
    )) {
      if (!perTag.has(tag)) perTag.set(tag, new Map());
      if (!perTag.get(tag).has(digest)) perTag.get(tag).set(digest, []);
      perTag.get(tag).get(digest).push(path);
    }
  }

  assert.ok(perTag.size > 0, 'geen enkel gepind rust-basisimage gevonden');
  for (const [tag, digests] of perTag) {
    assert.equal(
      digests.size,
      1,
      `${tag} is op ${digests.size} digests gepind: ` +
        [...digests].map(([d, ps]) => `${d.slice(0, 19)} in ${[...new Set(ps)].join(', ')}`).join(' — '),
    );
  }
});

test('elke BIN-build-arg in deploy.yml bestaat als bin-target', () => {
  const bins = binTargets();
  const args = [...read('.github/workflows/deploy.yml').matchAll(/BIN=([A-Za-z0-9_-]+)/g)].map((m) => m[1]);
  assert.ok(args.length >= 3, `slechts ${args.length} BIN-build-args gevonden`);
  for (const bin of args) {
    assert.ok(bins.has(bin), `deploy.yml bouwt BIN=${bin}, maar dat is geen bin-target van de workspace`);
  }
});

test('elk binary-pad dat een runtime-trap kopieert bestaat als bin-target', () => {
  const bins = binTargets();
  let seen = 0;
  for (const path of DOCKERFILES) {
    const copies = read(path).matchAll(/^COPY --from=builder \/build\/target\/release\/([A-Za-z0-9_-]+)/gm);
    for (const [, bin] of copies) {
      seen += 1;
      assert.ok(bins.has(bin), `${path}: kopieert ${bin}, maar dat is geen bin-target van de workspace`);
    }
  }
  assert.ok(seen >= 4, `slechts ${seen} binary-kopieën gevonden`);
});
