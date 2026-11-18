// Pins what script/schema-diff.mjs writes for a pull request that adds a schema
// version: the summary, the noise filter, the cut at the comment limit, and the
// empty output that tells the workflow there is nothing to post.
//
// The end-to-end cases build a throwaway git repository, because which version
// counts as new and which as previous comes from git, and a fixture that skips
// git would not test that part.

import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

import {
  MARKER,
  collectSections,
  compareSchemas,
  compareVersions,
  formatPath,
  previousVersion,
  renderComment,
} from './schema-diff.mjs';

const SCRIPT = fileURLToPath(new URL('./schema-diff.mjs', import.meta.url));

// A pre-commit hook runs with GIT_INDEX_FILE (and sometimes GIT_DIR) pointing
// at the repository being committed. Inherited by the throwaway repositories
// below, `git add` there would overwrite the index of that commit. Every git
// call in this file, the CLI's included, inherits process.env, so clear them
// once here.
for (const key of Object.keys(process.env)) {
  if (key.startsWith('GIT_')) delete process.env[key];
}

// --- Fixtures -----------------------------------------------------------------

/** A small schema in the shape of the real one, for the given version. */
function schema(version, extra = {}) {
  return {
    $schema: 'http://json-schema.org/draft-07/schema#',
    $id: `https://example.org/refs/tags/schema-${version}/schema/${version}/schema.json`,
    title: `Example Schema ${version}`,
    type: 'object',
    properties: {
      name: { type: 'string', description: 'Since v0.1.0: the name.' },
    },
    definitions: {
      field: {
        type: 'object',
        required: ['name'],
        properties: {
          name: { type: 'string' },
          type: { type: 'string', enum: ['boolean', 'number'] },
        },
      },
      ...extra,
    },
  };
}

/** v0.2.0 with a few changes: one definition and one property added, one enum value. */
function changedSchema() {
  const doc = schema('v0.2.0', { column: { type: 'object', properties: { name: { type: 'string' } } } });
  doc.definitions.field.properties.type.enum.push('array');
  doc.definitions.field.properties.nullable = { type: 'boolean' };
  return doc;
}

const compare = (oldDoc, newDoc, oldVersion = 'v0.1.0', newVersion = 'v0.2.0') =>
  compareSchemas({
    oldDoc,
    newDoc,
    oldVersion,
    newVersion,
    oldLabel: `schema/${oldVersion}/schema.json`,
    newLabel: `schema/${newVersion}/schema.json`,
  });

const section = (result, newVersion = 'v0.2.0', oldVersion = 'v0.1.0') => ({
  newVersion,
  oldVersion,
  isNew: true,
  label: `schema/${newVersion}/schema.json`,
  result,
});

// --- Versions -----------------------------------------------------------------

test('versions are ordered by number, not as text', () => {
  assert.ok(compareVersions('v0.8.10', 'v0.8.9') > 0);
  assert.ok(compareVersions('v1.0.0', 'v0.99.99') > 0);
  assert.equal(compareVersions('v0.8.19', 'v0.8.19'), 0);
});

test('the previous version is the highest one below', () => {
  const all = ['v0.8.9', 'v0.8.10', 'v0.8.19', 'v0.8.20', 'v0.9.0'];
  assert.equal(previousVersion('v0.8.20', all), 'v0.8.19');
  assert.equal(previousVersion('v0.8.10', all), 'v0.8.9');
  assert.equal(previousVersion('v0.8.9', all), null);
});

test('a path reads as the schema reader names it', () => {
  assert.equal(
    formatPath(['definitions', 'machineReadableSection', 'properties', 'projection', 'properties', 'state', 'items', 'properties', 'nullable']),
    'machineReadableSection.projection.state[].nullable',
  );
  assert.equal(formatPath(['definitions', 'parameterField', 'allOf', 0]), 'parameterField.allOf[0]');
  assert.equal(formatPath(['$id']), '$id');
  // `properties` as the last segment is a key in its own right, not a container.
  assert.equal(formatPath(['definitions', 'x', 'properties']), 'x.properties');
  // Directly under `properties` a key is a name, also `items` or `properties`.
  assert.equal(
    formatPath(['definitions', 'listOperation', 'properties', 'items', 'description']),
    'listOperation.items.description',
  );
  assert.equal(formatPath(['definitions', 'x', 'properties', 'properties', 'type']), 'x.properties.type');
  // So is a key directly under `definitions`.
  assert.equal(formatPath(['definitions', 'items', 'type']), 'items.type');
});

test('a property named like an Object method is compared as any other', () => {
  const oldDoc = schema('v0.1.0');
  const newDoc = schema('v0.2.0');
  oldDoc.definitions.field.properties.constructor = { type: 'string' };
  newDoc.definitions.field.properties.toString = { type: 'string' };
  const result = compare(oldDoc, newDoc);
  assert.deepEqual(result.summary.properties.removed, ['field.constructor']);
  assert.deepEqual(result.summary.properties.added, ['field.toString']);
});

test('a property named items counts as a property', () => {
  const oldDoc = schema('v0.1.0');
  const newDoc = schema('v0.2.0');
  newDoc.definitions.field.properties.items = { type: 'array' };
  assert.deepEqual(compare(oldDoc, newDoc).summary.properties.added, ['field.items']);
});

test('one branch inserted in a oneOf is one change, not a shift', () => {
  const oldDoc = schema('v0.1.0');
  oldDoc.definitions.operation = { oneOf: [{ const: 'ADD' }, { const: 'MAX' }, { const: 'MIN' }] };
  const newDoc = schema('v0.2.0');
  newDoc.definitions.operation = { oneOf: [{ const: 'ADD' }, { const: 'DIV' }, { const: 'MAX' }, { const: 'MIN' }] };
  assert.deepEqual(
    compare(oldDoc, newDoc).changes.map((c) => [c.kind, formatPath(c.path)]),
    [['added', 'operation.oneOf[1]']],
  );
  // At the end, and into an empty array.
  const atEnd = structuredClone(oldDoc);
  atEnd.definitions.operation.oneOf.push({ const: 'SUB' });
  assert.deepEqual(
    compare(oldDoc, { ...atEnd, $id: newDoc.$id, title: newDoc.title }).changes.map((c) => [c.kind, formatPath(c.path)]),
    [['added', 'operation.oneOf[3]']],
  );
  const fromEmpty = structuredClone(oldDoc);
  fromEmpty.definitions.operation.oneOf = [];
  const one = structuredClone(newDoc);
  one.definitions.operation.oneOf = [{ const: 'ADD' }];
  assert.deepEqual(
    compare(fromEmpty, one).changes.map((c) => [c.kind, formatPath(c.path)]),
    [['added', 'operation.oneOf[0]']],
  );
  // An insert plus another edit is no single insert: compared by position.
  const both = structuredClone(newDoc);
  both.definitions.operation.oneOf[3] = { const: 'MINIMUM' };
  assert.ok(compare(oldDoc, both).changes.length > 1);
  assert.deepEqual(
    compare(newDoc, oldDoc, 'v0.2.0', 'v0.1.0').changes.map((c) => [c.kind, formatPath(c.path)]),
    [['removed', 'operation.oneOf[1]']],
  );
});

test('a duplicate that comes or goes in an enum is a change', () => {
  const oldDoc = schema('v0.1.0');
  const newDoc = schema('v0.2.0');
  newDoc.definitions.field.properties.type.enum = ['boolean', 'boolean', 'number'];
  const change = compare(oldDoc, newDoc).changes[0];
  assert.deepEqual(change.added, ['boolean']);
  assert.equal(change.reordered, false);
});

test('a reordered enum says so', () => {
  const oldDoc = schema('v0.1.0');
  const newDoc = schema('v0.2.0');
  newDoc.definitions.field.properties.type.enum.reverse();
  const md = renderComment([section(compare(oldDoc, newDoc))]);
  assert.match(md, /- gewijzigd: `field\.type\.enum` \(alleen de volgorde\)/);
});

// --- A new version with a few changes ------------------------------------------

test('a new version with a few changes is summarised and diffed', () => {
  const result = compare(schema('v0.1.0'), changedSchema());

  assert.deepEqual(result.summary.definitions, { added: ['column'], removed: [], changed: ['field'] });
  assert.deepEqual(result.summary.properties.added, ['field.nullable']);
  assert.deepEqual(result.summary.properties.changed, ['field.type']);

  const enumChange = result.changes.find((c) => formatPath(c.path) === 'field.type.enum');
  assert.deepEqual(enumChange.added, ['array']);
  assert.deepEqual(enumChange.removed, []);

  // The noise is reported as noise and kept out of the diff.
  assert.deepEqual(result.noise.map(formatPath).sort(), ['$id', 'title']);
  assert.doesNotMatch(result.diff, /\$id|"title"/);
  assert.match(result.diff, /^\+\s+"array"$/m);
  assert.match(result.diff, /^--- schema\/v0\.1\.0\/schema\.json$/m);

  const md = renderComment([section(result)]);
  assert.ok(md.startsWith(`${MARKER}\n`), 'the marker is the first line');
  assert.match(md, /### `v0\.2\.0` \(nieuw\) ten opzichte van `v0\.1\.0`/);
  assert.match(md, /\| definities \| 1 \| 0 \| 1 \|/);
  assert.match(md, /\| eigenschappen \| 1 \| 0 \| 1 \|/);
  assert.match(md, /- toegevoegd: `column`/);
  assert.match(md, /- gewijzigd: `field\.type\.enum` \(\+ `"array"`\)/);
  assert.match(md, /<details>\n<summary>Diff van `schema\/v0\.2\.0\/schema\.json`/);
  assert.doesNotMatch(md, /Afgekapt/);
});

// --- Noise only ---------------------------------------------------------------

test('a version that differs only in version strings has no substantive change', () => {
  const result = compare(schema('v0.1.0'), schema('v0.2.0'));
  assert.deepEqual(result.changes, []);
  assert.equal(result.diff, '');

  const md = renderComment([section(result)]);
  assert.match(md, /\*\*Geen inhoudelijke wijziging\.\*\*/);
  assert.doesNotMatch(md, /```diff/);
});

test('a version mentioned in passing is not noise', () => {
  // "since v0.1.0" stays the same in the next version; rewriting every
  // version number would make it look changed.
  const oldDoc = schema('v0.1.0');
  const newDoc = schema('v0.2.0');
  newDoc.properties.name.description = 'Since v0.2.0: the name.';
  const result = compare(oldDoc, newDoc);
  assert.deepEqual(result.changes.map((c) => formatPath(c.path)), ['name.description']);
});

test('a URL deeper in the schema that only changes its version is noise', () => {
  const oldDoc = schema('v0.1.0');
  const newDoc = schema('v0.2.0');
  oldDoc.properties.name.$comment = 'https://example.org/schema/v0.1.0/schema.json';
  newDoc.properties.name.$comment = 'https://example.org/schema/v0.2.0/schema.json';
  const result = compare(oldDoc, newDoc);
  assert.deepEqual(result.changes, []);
  assert.ok(result.noise.map(formatPath).includes('name.$comment'));
});

test('a top-level description that only changes its version is noise', () => {
  // Top-level strings describe the document itself, so a version number
  // there names this version.
  const oldDoc = { ...schema('v0.1.0'), description: 'Example schema, version 0.1.0.' };
  const newDoc = { ...schema('v0.2.0'), description: 'Example schema, version 0.2.0.' };
  const result = compare(oldDoc, newDoc);
  assert.deepEqual(result.changes, []);
  assert.ok(result.noise.map(formatPath).includes('description'));
});

test('a longer version number is not mistaken for the old one', () => {
  const oldDoc = schema('v0.1.0');
  const newDoc = schema('v0.2.0');
  oldDoc.properties.name.description = 'See v0.1.01.';
  newDoc.properties.name.description = 'See v0.2.01.';
  assert.equal(compare(oldDoc, newDoc).changes.length, 1);
});

// --- Too large ----------------------------------------------------------------

test('a diff that is too large is cut at a line, with a note', () => {
  const oldDoc = schema('v0.1.0');
  const newDoc = schema('v0.2.0');
  for (let i = 0; i < 2000; i++) {
    newDoc.definitions[`def${i}`] = { type: 'string', description: `Definition number ${i}.` };
  }
  const result = compare(oldDoc, newDoc);
  const maxChars = 20000;
  const md = renderComment([section(result)], { maxChars });

  assert.ok(md.length <= maxChars, `comment is ${md.length} characters, over ${maxChars}`);
  assert.match(md, /\*\*Afgekapt:\*\* \d+ van \d+ regels getoond/);
  // The fence is closed and the block too, so the markdown below still renders.
  assert.match(md, /\n```\n\n\*\*Afgekapt/);
  assert.ok(md.trimEnd().endsWith('</details>'));
  // The path list is capped as well.
  assert.match(md, /- en nog \d+ paden; zie de diff/);
});

test('two diffs share the room, and the comment stays within any limit', () => {
  const big = (version) => {
    const doc = schema(version);
    for (let i = 0; i < 400; i++) doc.definitions[`def${i}`] = { type: 'string' };
    return doc;
  };
  const small = compare(schema('v0.1.0'), changedSchema());
  const large = compare(schema('v0.2.0'), big('v0.3.0'), 'v0.2.0', 'v0.3.0');
  const sections = [section(small), section(large, 'v0.3.0', 'v0.2.0')];

  const md = renderComment(sections, { maxChars: 12000 });
  assert.ok(md.length <= 12000, `comment is ${md.length} characters`);
  // The small diff is shown whole, the large one is cut.
  assert.equal(md.match(/\*\*Afgekapt:\*\*/g)?.length, 1);
  assert.match(md, /^\+\s+"array"$/m);

  // Down to the point where only the summaries fit, the limit holds exactly.
  const summariesOnly = sections.map((s) => ({ ...s, result: { ...s.result, diff: '' } }));
  const heads = renderComment(summariesOnly).length;
  for (let max = heads; max < heads + 3000; max += 7) {
    const out = renderComment(sections, { maxChars: max });
    assert.ok(out.length <= max, `maxChars ${max} gave ${out.length}`);
  }
});

test('the default limit stays under the GitHub comment limit', () => {
  const oldDoc = schema('v0.1.0');
  const newDoc = schema('v0.2.0');
  for (let i = 0; i < 20000; i++) newDoc.definitions[`def${i}`] = { type: 'string' };
  const md = renderComment([section(compare(oldDoc, newDoc))]);
  assert.ok(md.length <= 65536, `comment is ${md.length} characters`);
});

test('a diff containing a code fence does not break out of its block', () => {
  const oldDoc = schema('v0.1.0');
  const newDoc = schema('v0.2.0');
  newDoc.properties.name.description = 'Example:\n```yaml\na: 1\n```';
  const md = renderComment([section(compare(oldDoc, newDoc))]);
  assert.match(md, /^````diff$/m);
});

// --- End to end, through git ----------------------------------------------------

const repos = [];
after(() => {
  for (const dir of repos) rmSync(dir, { recursive: true, force: true });
});

function gitRepo() {
  const dir = mkdtempSync(join(tmpdir(), 'schema-diff-test-'));
  repos.push(dir);
  const git = (...args) =>
    execFileSync('git', ['-c', 'user.name=Test', '-c', 'user.email=test@example.org', '-c', 'commit.gpgsign=false', ...args], {
      cwd: dir,
      encoding: 'utf8',
    }).trim();
  git('init', '-q', '-b', 'main');
  const write = (path, content) => {
    mkdirSync(join(dir, path, '..'), { recursive: true });
    writeFileSync(join(dir, path), typeof content === 'string' ? content : `${JSON.stringify(content, null, 2)}\n`);
  };
  const commit = (message) => {
    git('add', '-A');
    git('commit', '-q', '-m', message);
    return git('rev-parse', 'HEAD');
  };
  return { dir, git, write, commit };
}

function runCli(dir, base, head) {
  return execFileSync('node', [SCRIPT, '--base', base, '--head', head], { cwd: dir, encoding: 'utf8' });
}

test('a pull request that adds no schema version gets no comment', () => {
  const repo = gitRepo();
  repo.write('schema/v0.1.0/schema.json', schema('v0.1.0'));
  const base = repo.commit('base');
  repo.write('README.md', 'other change\n');
  // Touching a file under schema/ that is not a version must not count either.
  repo.write('schema/README.md', 'notes\n');
  const head = repo.commit('head');

  assert.equal(runCli(repo.dir, base, head), '');
});

test('a pull request that adds a version is compared to the highest one below it', () => {
  const repo = gitRepo();
  repo.write('schema/v0.1.0/schema.json', schema('v0.1.0'));
  repo.write('schema/v0.1.9/schema.json', schema('v0.1.9'));
  const base = repo.commit('base');
  repo.write('schema/v0.2.0/schema.json', changedSchema());
  const head = repo.commit('head');

  const md = runCli(repo.dir, base, head);
  assert.match(md, /### `v0\.2\.0` \(nieuw\) ten opzichte van `v0\.1\.9`/);
  assert.match(md, /--- schema\/v0\.1\.9\/schema\.json/);
});

test('two new versions in one pull request are each compared to their predecessor', () => {
  const repo = gitRepo();
  repo.write('schema/v0.1.0/schema.json', schema('v0.1.0'));
  const base = repo.commit('base');
  repo.write('schema/v0.2.0/schema.json', schema('v0.2.0'));
  repo.write('schema/v0.3.0/schema.json', changedSchema());
  const head = repo.commit('head');

  const sections = collectSections({ base, head, cwd: repo.dir });
  assert.deepEqual(
    sections.map((s) => [s.newVersion, s.oldVersion]),
    [['v0.2.0', 'v0.1.0'], ['v0.3.0', 'v0.2.0']],
  );
  assert.equal(sections[0].result.changes.length, 0);
  assert.ok(sections[1].result.changes.length > 0);
});

test('an edited released version is compared to its own copy on the base', () => {
  const repo = gitRepo();
  repo.write('schema/v0.1.0/schema.json', schema('v0.1.0'));
  repo.write('schema/v0.2.0/schema.json', schema('v0.2.0'));
  const base = repo.commit('base');
  const edited = schema('v0.2.0');
  edited.type = 'array';
  repo.write('schema/v0.2.0/schema.json', edited);
  const head = repo.commit('head');

  const [s] = collectSections({ base, head, cwd: repo.dir });
  assert.equal(s.isNew, false);
  assert.equal(s.oldVersion, 'v0.2.0');
  assert.deepEqual(s.result.changes.map((c) => formatPath(c.path)), ['type']);
  assert.match(renderComment([s]), /\(gewijzigd\).*\n\n`v0\.2\.0` staat al op de base-branch/);
});

test('the command the truncation note names gives the whole diff', () => {
  const repo = gitRepo();
  repo.write('schema/v0.1.0/schema.json', schema('v0.1.0'));
  const base = repo.commit('base');
  const big = schema('v0.2.0');
  for (let i = 0; i < 4000; i++) big.definitions[`def${i}`] = { type: 'string', description: `Definition ${i}.` };
  repo.write('schema/v0.2.0/schema.json', big);
  const head = repo.commit('head');

  const cut = execFileSync('node', [SCRIPT, '--base', base, '--head', head, '--max-chars', '60000'], {
    cwd: repo.dir,
    encoding: 'utf8',
  });
  assert.match(cut, /\*\*Afgekapt:\*\*/);
  assert.ok(cut.length <= 60000);
  assert.doesNotMatch(runCli(repo.dir, base, head), /Afgekapt/);
});

test('the first schema version has nothing to compare to', () => {
  const repo = gitRepo();
  repo.write('README.md', 'start\n');
  const base = repo.commit('base');
  repo.write('schema/v0.1.0/schema.json', schema('v0.1.0'));
  const head = repo.commit('head');

  const md = runCli(repo.dir, base, head);
  assert.match(md, /Er is geen eerdere schemaversie om mee te vergelijken\./);
});
