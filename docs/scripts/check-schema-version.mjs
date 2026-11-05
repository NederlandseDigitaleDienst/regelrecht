// Assert the schema reference page tracks the latest released schema version.
//
// `reference/schema.mdx` hand-states the current schema version in three
// places (a "current version is vX.Y.Z" line, an immutable-tag example URL,
// and the top row of the Version History table), and renders the rest of the
// page from a committed snapshot of the schema, which is the fourth thing
// checked here. None of them are derived from the repo at build time, so the
// page silently rots the moment `schema/latest` is bumped and
// nobody edits the doc: it then advertises an old version as current while
// looking authoritative, and law authors copy a stale `$schema` URL. This
// check makes CI fail when the page falls behind `schema/latest`, so the
// versioned schema and its documentation can never drift apart again.
//
// It reads source files only (the `schema/latest` symlink + the markdown), not
// the astro build, so it runs without `astro build`. The `schema/` tree lives
// outside docs/ and is not in the docs Docker build context, but it IS present
// in the CI repo checkout where this script runs (see the provenance-checks
// job). Non-zero exit fails the gate.

import { readdirSync, readFileSync, existsSync, lstatSync, readlinkSync } from 'node:fs';
import { basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const SCHEMA_DIR = fileURLToPath(new URL('../../schema', import.meta.url));
const PAGE = fileURLToPath(
  new URL('../src/content/docs/reference/schema.mdx', import.meta.url),
);
// One note per schema version from RELEASE_NOTES_FROM on, which the Version
// History rows link to. See check 6.
const NOTES = fileURLToPath(
  new URL('../src/content/docs/reference/schema-release-notes.mdx', import.meta.url),
);
const RELEASE_NOTES_FROM = [0, 8, 0];
// The committed copy the reference page actually renders from. It exists
// because `schema/` is not in the docs Docker build context; see
// scripts/sync-schema.mjs.
const SNAPSHOT = fileURLToPath(
  new URL('../src/data/schema-latest.json', import.meta.url),
);

if (!existsSync(SCHEMA_DIR)) {
  // Outside the CI checkout (e.g. the docs-only Docker build context) the
  // schema tree is absent. Skip rather than fail: this guard is a CI gate, not
  // a build step, and there is nothing to compare against here.
  console.log('check-schema-version: schema/ not present, skipping (not a full-repo checkout)');
  process.exit(0);
}

// The released version is whatever schema/latest resolves to — the single
// source of truth the rest of the repo already keys off (provenance-checks
// asserts the symlink points at the highest schema/vX.Y.Z directory).
const latestLink = `${SCHEMA_DIR}/latest`;
let latest;
if (existsSync(latestLink) && lstatSync(latestLink).isSymbolicLink()) {
  latest = basename(readlinkSync(latestLink));
} else {
  // Fall back to the highest versioned directory (e.g. checkouts that
  // materialize symlinks as plain dirs).
  latest = readdirSync(SCHEMA_DIR)
    .filter((n) => /^v\d+\.\d+\.\d+$/.test(n))
    .sort((a, b) =>
      a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' }),
    )
    .at(-1);
}

if (!latest || !/^v\d+\.\d+\.\d+$/.test(latest)) {
  console.error(`check-schema-version FAILED: could not determine latest schema version from ${SCHEMA_DIR}`);
  process.exit(1);
}

const page = readFileSync(PAGE, 'utf8');
const problems = [];

// 1. The prose "current version is vX.Y.Z" line must name the latest version.
const currentLine = page.match(/current schema version is \*\*(v\d+\.\d+\.\d+)\*\*/);
if (!currentLine) {
  problems.push('could not find the "current schema version is **vX.Y.Z**" line');
} else if (currentLine[1] !== latest) {
  problems.push(
    `"current schema version" says ${currentLine[1]} but schema/latest is ${latest}`,
  );
}

// 2. The example immutable-tag URL must point at the latest version's tag+dir.
const expectedUrl = `https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-${latest}/schema/${latest}/schema.json`;
if (!page.includes(expectedUrl)) {
  problems.push(`the example schema URL does not point at ${latest} (expected ${expectedUrl})`);
}

// 3. The Version History table must have a row for the latest version, so the
//    thing that changed is actually described (with its RFC). `latest` is a
//    validated vX.Y.Z string, but escape every regex metacharacter (not just
//    the dots) so the pattern stays literal regardless of its shape.
const literal = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const escaped = literal(latest);
// The version cell may be a link to its release note (check 6).
const historyRow = new RegExp(`^\\|\\s*(?:${escaped}|\\[${escaped}\\]\\([^)]*\\))\\s*\\|`, 'm');
if (!historyRow.test(page)) {
  problems.push(`the Version History table has no row for ${latest}`);
}

// 4. The committed snapshot the page renders from must BE `schema/latest`.
//    Byte-for-byte: the page shows field names, descriptions and examples
//    straight out of this file, so a stale copy does not merely misstate a
//    version number, it documents a schema that is no longer current.
let snapshotStale = false;
if (!existsSync(SNAPSHOT)) {
  problems.push(
    'docs/src/data/schema-latest.json is missing (run: npm run sync:schema)',
  );
  snapshotStale = true;
} else {
  const snapshot = readFileSync(SNAPSHOT);
  const source = readFileSync(`${SCHEMA_DIR}/latest/schema.json`);
  if (!snapshot.equals(source)) {
    // Name the version the snapshot claims, so the failure says what drifted
    // rather than only that something did.
    let snapshotVersion = 'unparseable';
    try {
      snapshotVersion = JSON.parse(snapshot.toString('utf8')).title ?? 'untitled';
    } catch {
      /* keep the placeholder */
    }
    problems.push(
      `docs/src/data/schema-latest.json does not match schema/latest (${latest}); ` +
        `the snapshot says "${snapshotVersion}"`,
    );
    snapshotStale = true;
  }
}

// 5. The tag the page advertises must exist. The URL in `$schema` is a
//    promise to every law author and every outside reader that the schema
//    they validated against stays fetchable at that address. Shape alone is
//    not enough: the corpus check elsewhere matches `$schema` against the
//    local `schema/vX.Y.Z` directories, so a version that was released in the
//    tree but never tagged passes every existing gate while its published URL
//    404s. That is how five versions shipped untagged (v0.5.7 through v0.7.0),
//    leaving eleven corpus files citing an address that returned a 404. They
//    have since been tagged; this check is what keeps the next one from
//    slipping out the same way.
//
//    This blocks. Tagging is the step that makes a released version immutable:
//    without a tag the `$schema` URL resolves to nothing, and nothing stops the
//    file being edited afterwards, so "a published version is never modified"
//    holds by convention rather than by construction. A release is not finished
//    until its tag exists.
//
//    Tags are only present in a checkout that fetched them, so a missing tag
//    list is not evidence of a missing tag: skip when there are none at all
//    rather than fail a shallow clone. Use `fetch-depth: 0` in CI, or fetch the
//    tags locally, to make this check meaningful.
//
//    A version that is being released right now has no tag yet, and cannot
//    have one: `.github/workflows/tag-schema.yml` sets it on the push to main,
//    because a tag on a pull request's test merge would point at a commit that
//    never becomes main. So a version counts as released, and must be tagged,
//    only once it is on main and was already there before the commit under
//    test. That is "present at origin/main" (a pull request, the merge queue,
//    a local branch) and "present at HEAD^" (the push to main that releases
//    it, where tag-schema runs in parallel with this check). A version
//    missing from either is the release in progress.
try {
  const { execFileSync } = await import('node:child_process');
  const git = (args) =>
    execFileSync('git', args, {
      cwd: SCHEMA_DIR,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  const tags = git(['tag', '--list', 'schema-v*'])
    .split('\n')
    .map((t) => t.trim())
    .filter(Boolean);

  // The schema versions present at a ref, or null when the ref cannot be read
  // (a shallow clone without HEAD^, a checkout without origin/main).
  const versionsAt = (ref) => {
    try {
      return new Set(
        git(['ls-tree', '--full-tree', '--name-only', `${ref}:schema`])
          .split('\n')
          .map((n) => n.trim())
          .filter((n) => /^v\d+\.\d+\.\d+$/.test(n)),
      );
    } catch {
      return null;
    }
  };
  const onMain = versionsAt('refs/remotes/origin/main');
  const beforeHead = versionsAt('HEAD^');
  const inRelease = (v) =>
    (onMain !== null && !onMain.has(v)) || (beforeHead !== null && !beforeHead.has(v));

  if (tags.length === 0) {
    console.warn(
      'check-schema-version: no schema-v* tags in this checkout, skipping the tag check (run `git fetch --tags`)',
    );
  } else {
    // Report every untagged released version, not just the latest: the gap is
    // historical (v0.5.7 onwards) and a law file may cite any of them.
    const present = readdirSync(SCHEMA_DIR).filter((n) => /^v\d+\.\d+\.\d+$/.test(n));
    const pending = present.filter(inRelease);
    const released = present.filter((v) => !inRelease(v));
    const untagged = released.filter((v) => !tags.includes(`schema-${v}`)).sort();
    for (const v of pending) {
      if (!tags.includes(`schema-${v}`)) {
        console.log(
          `check-schema-version: ${v} is being released (not on origin/main, or added by this ` +
            `commit); tag-schema.yml tags it on the push to main`,
        );
      }
    }
    if (untagged.length) {
      problems.push(
        `${untagged.length} released schema version(s) have no tag, so the $schema URL a law ` +
          `file cites for them does not resolve: ${untagged.join(', ')}. ` +
          `A release is finished when its tag exists: tag each one at the commit ` +
          `that released it, then push the tag.`,
      );
    }
  }
} catch {
  console.warn('check-schema-version: could not list git tags, skipping the tag check');
}

// 6. Every schema version from v0.8.0 on has a release note, and its row in
//    the Version History links to that note. The note is where an author
//    reads whether a version of a rulework stays valid and what to edit when
//    it does not; the row stays the record RFCs cite. Without this check the
//    pull request that adds a version can add the row and forget the note,
//    and nothing would notice until a reader followed a link to nowhere.
//    The anchor is the slug Astro gives a heading that is the version alone:
//    `## v0.8.13` becomes `#v0813`.
const parse = (v) => v.slice(1).split('.').map(Number);
const atLeast = (a, b) => {
  for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] > b[i];
  return true;
};
const noted = readdirSync(SCHEMA_DIR)
  .filter((n) => /^v\d+\.\d+\.\d+$/.test(n))
  .filter((v) => atLeast(parse(v), RELEASE_NOTES_FROM));
const notes = existsSync(NOTES) ? readFileSync(NOTES, 'utf8') : '';
if (!notes) problems.push('docs/src/content/docs/reference/schema-release-notes.mdx is missing');
for (const v of noted) {
  if (notes && !new RegExp(`^## ${literal(v)}\\s*$`, 'm').test(notes)) {
    problems.push(`schema-release-notes.mdx has no "## ${v}" note`);
  }
  const link = `[${v}](/reference/schema-release-notes#${v.replace(/\./g, '')})`;
  if (!new RegExp(`^\\|\\s*${literal(link)}\\s*\\|`, 'm').test(page)) {
    problems.push(`the Version History row for ${v} does not link its release note as ${link}`);
  }
}

if (problems.length) {
  console.error(`check-schema-version FAILED (schema/latest is ${latest}):`);
  for (const p of problems) console.error('  - ' + p);
  if (snapshotStale) {
    console.error('\nRun `npm run sync:schema` to refresh the snapshot.');
  }
  console.error(
    '\nUpdate docs/src/content/docs/reference/schema.mdx: bump the current-version line ' +
      'and tag URL, and add a Version History row describing what the new version introduces, ' +
      'linked to a note in schema-release-notes.mdx (see "Adding a release note" there).',
  );
  process.exit(1);
}

console.log(`check-schema-version passed: reference page tracks schema/latest (${latest}).`);
