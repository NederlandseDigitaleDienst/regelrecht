// Shows what a new schema version changes compared to the version before it,
// as markdown for a pull request comment.
//
// A new schema version is a full copy: `schema/vX.Y.Z/schema.json` in a new
// directory. The pull request diff therefore shows a new file of some 3,700
// lines, while the change to the language is often a hundred or two. This
// script finds that change and writes it out twice:
//
// - a semantic diff: the JSON paths that were added, removed or changed, and a
//   count of definitions and properties per kind. That is what a reviewer reads
//   first, and it does not depend on formatting.
// - a `diff -u` of the two files, with the noise taken out, because the
//   semantic diff says *where* something changed and only the text shows a
//   rewritten description.
//
// The noise is a value that names the document's own version and differs only
// in that number: a top-level string (`$id`, `title`) or a URL anywhere. It is
// neutralised value by value: an old string counts as noise only when
// replacing the old version by the new one in it gives exactly the new string.
// Replacing every version number in the file would be wrong, because a
// description that says "since v0.8.16" stays the same in the next version. A
// description deeper in the schema that goes from "since v0.8.19" to "since
// v0.8.20" is a real change, so outside the top level only URLs qualify.
//
// Usage:
//   node script/schema-diff.mjs --base <ref> [--head <ref>] [--max-chars <n>]
//
// Prints the comment to stdout, or nothing when the head adds or changes no
// schema version. `.github/workflows/schema-diff.yml` posts it.

import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The line that identifies the comment, so a new push updates it. */
export const MARKER = '<!-- schema-diff -->';

/** GitHub refuses a comment over 65,536 characters; stay well below it. */
export const DEFAULT_MAX_CHARS = 60000;

/** At most this many paths are listed per version; the rest are counted. */
const MAX_PATHS = 150;

const VERSION_DIR = /^schema\/(v\d+\.\d+\.\d+)\/schema\.json$/;

// --- Versions -----------------------------------------------------------------

/** Parses `v1.2.3` into [1, 2, 3]. */
function parseVersion(version) {
  const m = /^v(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!m) throw new Error(`not a schema version: ${version}`);
  return m.slice(1).map(Number);
}

/** Orders versions numerically, so v0.8.10 comes after v0.8.9. */
export function compareVersions(a, b) {
  const pa = parseVersion(a);
  const pb = parseVersion(b);
  for (let i = 0; i < 3; i++) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i];
  }
  return 0;
}

/** The highest version in `available` below `version`, or null. */
export function previousVersion(version, available) {
  const lower = available.filter((v) => compareVersions(v, version) < 0);
  lower.sort(compareVersions);
  return lower.at(-1) ?? null;
}

// --- Semantic diff ------------------------------------------------------------

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const isScalar = (v) => v === null || typeof v !== 'object';

function deepEqual(a, b) {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** A regex matching `version` without its `v`, not as part of a longer number. */
function versionPattern(version) {
  const bare = version.replace(/^v/, '').replace(/\./g, '\\.');
  return new RegExp(`(?<![\\d.])${bare}(?![\\d])`, 'g');
}

/**
 * Returns a copy of `oldDoc` in which every top-level string or URL that
 * differs from its counterpart in `newDoc` only by the version number is
 * replaced by that counterpart, plus the paths where that happened.
 */
export function neutraliseVersionNoise(oldDoc, newDoc, oldVersion, newVersion) {
  const pattern = versionPattern(oldVersion);
  const bareNew = newVersion.replace(/^v/, '');
  const noise = [];

  function walk(a, b, path) {
    if (typeof a === 'string' && typeof b === 'string') {
      const namesDocument = path.length === 1 || /^https?:\/\//.test(a);
      if (namesDocument && a !== b && a.replace(pattern, bareNew) === b) {
        noise.push(path);
        return b;
      }
      return a;
    }
    if (isObject(a) && isObject(b)) {
      const out = {};
      for (const key of Object.keys(a)) {
        out[key] = Object.hasOwn(b, key) ? walk(a[key], b[key], [...path, key]) : a[key];
      }
      return out;
    }
    if (Array.isArray(a) && Array.isArray(b)) {
      return a.map((item, i) => (i < b.length ? walk(item, b[i], [...path, i]) : item));
    }
    return a;
  }

  return { doc: walk(oldDoc, newDoc, []), noise };
}

/**
 * The differences between two JSON values, as a list of
 * `{ kind: 'added' | 'removed' | 'changed', path }`. An added or removed key is
 * reported once, not once per value inside it. An array of scalars (`enum`,
 * `required`) is compared as a set, so adding one item does not report every
 * item after it as changed.
 */
export function semanticDiff(a, b, path = [], out = []) {
  if (deepEqual(a, b)) return out;

  if (isObject(a) && isObject(b)) {
    for (const key of Object.keys(a)) {
      if (!Object.hasOwn(b, key)) out.push({ kind: 'removed', path: [...path, key] });
      else semanticDiff(a[key], b[key], [...path, key], out);
    }
    for (const key of Object.keys(b)) {
      if (!Object.hasOwn(a, key)) out.push({ kind: 'added', path: [...path, key] });
    }
    return out;
  }

  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.every(isScalar) && b.every(isScalar)) {
      const added = b.filter((x) => !a.some((y) => y === x));
      const removed = a.filter((x) => !b.some((y) => y === x));
      out.push({ kind: 'changed', path, added, removed, reordered: added.length + removed.length === 0 });
      return out;
    }
    for (let i = 0; i < Math.max(a.length, b.length); i++) {
      if (i >= b.length) out.push({ kind: 'removed', path: [...path, i] });
      else if (i >= a.length) out.push({ kind: 'added', path: [...path, i] });
      else semanticDiff(a[i], b[i], [...path, i], out);
    }
    return out;
  }

  out.push({ kind: 'changed', path });
  return out;
}

/**
 * Whether the segment at `i` is a JSON Schema keyword rather than a property
 * name. Directly under a `properties` keyword every key is a name, also one
 * called `items` or `properties` (the schema has both).
 */
function isKeyword(path, i) {
  return i === 0 || path[i - 1] !== 'properties' || !isKeyword(path, i - 1);
}

/**
 * A path as a reader of the schema names it: without the `definitions.` in
 * front, with `.properties.` dropped and `items` written as `[]`. So
 * `definitions.machineReadableSection.properties.projection.properties.state.items`
 * becomes `machineReadableSection.projection.state[]`.
 */
export function formatPath(path) {
  let out = '';
  path.forEach((seg, i) => {
    if (i === 0 && seg === 'definitions' && path.length > 1) return;
    if (seg === 'properties' && isKeyword(path, i) && i < path.length - 1) return;
    if (seg === 'items' && isKeyword(path, i) && i > 0) {
      out += '[]';
      return;
    }
    if (typeof seg === 'number') {
      out += `[${seg}]`;
      return;
    }
    const name = /^[A-Za-z_$][\w$-]*$/.test(seg) ? seg : JSON.stringify(seg);
    out += out === '' ? name : `.${name}`;
  });
  return out === '' ? '(root)' : out;
}

/**
 * Counts definitions and properties that were added, removed or changed. A
 * definition is changed when anything inside it changed; a property is added
 * or removed when its key under a `properties` object is, and changed when
 * something inside it changed. Properties inside an added definition are part
 * of that definition and not counted again.
 */
export function summarise(changes) {
  const defs = { added: new Set(), removed: new Set(), changed: new Set() };
  const props = { added: new Set(), removed: new Set(), changed: new Set() };

  for (const change of changes) {
    const { kind, path } = change;
    if (path[0] === 'definitions' && path.length >= 2) {
      const name = path[1];
      if (path.length === 2) defs[kind].add(name);
      else defs.changed.add(name);
    }
    if (path.length === 2 && path[0] === 'definitions') continue;

    const isProperties = (j) => path[j] === 'properties' && isKeyword(path, j);
    if (path.length >= 2 && isProperties(path.length - 2) && kind !== 'changed') {
      props[kind].add(formatPath(path));
      continue;
    }
    // The innermost property this change lies inside, if any.
    let last = path.length - 2;
    while (last >= 0 && !isProperties(last)) last--;
    if (last >= 0) props.changed.add(formatPath(path.slice(0, last + 2)));
  }
  for (const name of [...defs.added, ...defs.removed]) defs.changed.delete(name);

  const sorted = (set) => [...set].sort();
  return {
    definitions: {
      added: sorted(defs.added),
      removed: sorted(defs.removed),
      changed: sorted(defs.changed),
    },
    properties: {
      added: sorted(props.added),
      removed: sorted(props.removed),
      changed: sorted(props.changed),
    },
  };
}

// --- Text diff ----------------------------------------------------------------

/** `diff -u` of two texts, with the given labels, and the header lines kept. */
export function unifiedDiff(oldText, newText, oldLabel, newLabel) {
  const dir = mkdtempSync(join(tmpdir(), 'schema-diff-'));
  try {
    writeFileSync(join(dir, 'old'), oldText);
    writeFileSync(join(dir, 'new'), newText);
    const res = spawnSync(
      'diff',
      ['-u', '--label', oldLabel, '--label', newLabel, join(dir, 'old'), join(dir, 'new')],
      { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 },
    );
    // diff exits 1 when the files differ; 2 is trouble.
    if (res.status !== 0 && res.status !== 1) {
      throw new Error(`diff failed (${res.status}): ${res.stderr || res.error}`);
    }
    return res.stdout;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** The JSON as the repo formats it, so the line diff matches the files. */
const pretty = (doc) => `${JSON.stringify(doc, null, 2)}\n`;

// --- Comparing two schema versions -------------------------------------------

/**
 * Everything the comment says about one version: the semantic changes with
 * the noise left out, the paths that were only noise, a summary and the line
 * diff of the neutralised old file against the new one.
 */
export function compareSchemas({ oldDoc, newDoc, oldVersion, newVersion, oldLabel, newLabel }) {
  const { doc: neutralOld, noise } = neutraliseVersionNoise(oldDoc, newDoc, oldVersion, newVersion);
  const changes = semanticDiff(neutralOld, newDoc);
  const diff = changes.length === 0
    ? ''
    : unifiedDiff(pretty(neutralOld), pretty(newDoc), oldLabel, newLabel);
  return { changes, noise, summary: summarise(changes), diff };
}

// --- Markdown -----------------------------------------------------------------

const code = (text) => `\`${text}\``;

/** A code fence longer than any run of backticks in `text`. */
function fenceFor(text) {
  const longest = Math.max(2, ...(text.match(/`+/g) ?? []).map((m) => m.length));
  return '`'.repeat(longest + 1);
}

function nameList(names, max = 8) {
  const shown = names.slice(0, max).map(code).join(', ');
  return names.length > max ? `${shown} en ${names.length - max} meer` : shown;
}

function summaryTable(summary) {
  const row = (label, s) =>
    `| ${label} | ${s.added.length} | ${s.removed.length} | ${s.changed.length} |`;
  return [
    '| | toegevoegd | verwijderd | gewijzigd |',
    '|---|---:|---:|---:|',
    row('definities', summary.definitions),
    row('eigenschappen', summary.properties),
  ].join('\n');
}

function definitionLines(summary) {
  const d = summary.definitions;
  const lines = [];
  if (d.added.length) lines.push(`- Definities toegevoegd: ${nameList(d.added)}`);
  if (d.removed.length) lines.push(`- Definities verwijderd: ${nameList(d.removed)}`);
  if (d.changed.length) lines.push(`- Definities gewijzigd: ${nameList(d.changed)}`);
  return lines;
}

const KIND_LABEL = { added: 'toegevoegd', removed: 'verwijderd', changed: 'gewijzigd' };

function pathLines(changes) {
  const lines = changes.slice(0, MAX_PATHS).map((c) => {
    let line = `- ${KIND_LABEL[c.kind]}: ${code(formatPath(c.path))}`;
    const extra = [];
    const values = (xs) => nameList(xs.map((x) => JSON.stringify(x)), 10);
    if (c.added?.length) extra.push(`+ ${values(c.added)}`);
    if (c.removed?.length) extra.push(`- ${values(c.removed)}`);
    if (c.reordered) extra.push('alleen de volgorde');
    if (extra.length) line += ` (${extra.join('; ')})`;
    return line;
  });
  if (changes.length > MAX_PATHS) {
    lines.push(`- en nog ${changes.length - MAX_PATHS} paden; zie de diff`);
  }
  return lines;
}

/** The part of a section above the line diff, which is never cut. */
function sectionHead(section) {
  const { newVersion, oldVersion, isNew, result } = section;
  const lines = [];
  const status = isNew ? 'nieuw' : 'gewijzigd';
  if (oldVersion) {
    lines.push(`### ${code(newVersion)} (${status}) ten opzichte van ${code(oldVersion)}`);
  } else {
    lines.push(`### ${code(newVersion)} (${status})`, '', 'Er is geen eerdere schemaversie om mee te vergelijken.');
    return lines.join('\n');
  }
  if (!isNew) {
    lines.push(
      '',
      `${code(newVersion)} staat al op de base-branch. Een uitgebrachte versie verandert niet; ` +
        'een correctie wordt een nieuwe patchversie. De diff hieronder is tegen de versie op de base-branch.',
    );
  }
  lines.push('');

  if (result.noise.length) {
    const names = result.noise.map(formatPath);
    lines.push(
      `Alleen het versienummer verschilt in ${result.noise.length} ${result.noise.length === 1 ? 'waarde' : 'waarden'} ` +
        `(${nameList(names, 5)}); die ${result.noise.length === 1 ? 'telt' : 'tellen'} niet mee en ${result.noise.length === 1 ? 'staat' : 'staan'} niet in de diff.`,
      '',
    );
  }

  if (result.changes.length === 0) {
    lines.push('**Geen inhoudelijke wijziging.**');
    return lines.join('\n');
  }

  lines.push(summaryTable(result.summary), '');
  const defs = definitionLines(result.summary);
  if (defs.length) lines.push(...defs, '');
  lines.push(
    '<details>',
    `<summary>Paden met een wijziging (${result.changes.length})</summary>`,
    '',
    ...pathLines(result.changes),
    '',
    '</details>',
  );
  return lines.join('\n');
}

/** The line diff in a collapsible block, cut to at most `budget` characters. */
function diffBlock(diff, budget, label) {
  const all = diff.replace(/\n$/, '').split('\n');
  const fence = fenceFor(diff);
  const wrap = (body, note) =>
    [
      '<details>',
      `<summary>Diff van ${code(label)} (${all.length} regels)</summary>`,
      '',
      `${fence}diff`,
      body,
      fence,
      ...(note ? ['', note] : []),
      '',
      '</details>',
    ].join('\n');

  const full = wrap(all.join('\n'));
  if (full.length <= budget) return full;

  // Find the most lines that fit, note included.
  const note = (shown) =>
    `**Afgekapt:** ${shown} van ${all.length} regels getoond, omdat een comment op GitHub ` +
    'een maximale lengte heeft. De volledige diff: `node script/schema-diff.mjs --base origin/main`.';
  // Not even an empty block fits: name the diff and leave it out.
  const empty = wrap('', note(all.length)).length;
  if (empty > budget) {
    return `Diff van ${code(label)} (${all.length} regels) past niet meer in deze comment. ` +
      'De volledige diff: `node script/schema-diff.mjs --base origin/main`.';
  }
  let shown = 0;
  let used = empty;
  while (shown < all.length && used + all[shown].length + 1 <= budget) {
    used += all[shown].length + 1;
    shown++;
  }
  return wrap(all.slice(0, shown).join('\n'), note(shown));
}

/**
 * The whole comment for a list of sections, at most `maxChars` long as long
 * as the summaries fit; they are never cut, and the default limit leaves them
 * ample room. The line diffs share what room is left, a short diff
 * giving its unused share to the longer ones.
 */
export function renderComment(sections, { maxChars = DEFAULT_MAX_CHARS } = {}) {
  if (sections.length === 0) return '';

  const header = [
    MARKER,
    '## Schemadiff',
    '',
    'Wat de nieuwe schemaversie verandert ten opzichte van de vorige. De wijziging zelf, ' +
      'in gewone taal, staat in de beschrijving van de pull request. Deze comment wordt bij ' +
      'elke push bijgewerkt.',
  ].join('\n');

  const heads = sections.map(sectionHead);
  const withDiff = sections
    .map((s, i) => ({ i, diff: s.result.diff, label: s.label }))
    .filter((s) => s.diff !== '');

  // Joins: two newlines before each section and its diff, one at the end.
  const fixed = header.length + heads.reduce((n, h) => n + h.length + 2, 0) + withDiff.length * 2 + 1;
  let room = Math.max(0, maxChars - fixed);

  // Smallest first, so a short diff is shown whole and its leftover share
  // goes to the next one.
  const blocks = new Map();
  const bySize = [...withDiff].sort((a, b) => a.diff.length - b.diff.length);
  bySize.forEach((s, n) => {
    const share = Math.floor(room / (bySize.length - n));
    const block = diffBlock(s.diff, share, s.label);
    blocks.set(s.i, block);
    room -= block.length;
  });

  const body = sections.map((_, i) =>
    blocks.has(i) ? `${heads[i]}\n\n${blocks.get(i)}` : heads[i],
  );
  return `${[header, ...body].join('\n\n')}\n`;
}

// --- Git ----------------------------------------------------------------------

function git(args, opts = {}) {
  return execFileSync('git', args, {
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
    ...opts,
  });
}

/** The schema versions present at `ref`, with the blob id of each schema.json. */
export function versionsAt(ref, cwd) {
  const out = git(['ls-tree', '-r', ref, '--', 'schema/'], { cwd });
  const versions = new Map();
  for (const line of out.split('\n')) {
    // <mode> <type> <object>\t<path>
    const m = /^\d+ blob ([0-9a-f]+)\t(.+)$/.exec(line);
    if (!m) continue;
    const v = VERSION_DIR.exec(m[2]);
    if (v) versions.set(v[1], m[1]);
  }
  return versions;
}

function readSchema(ref, version, cwd) {
  const text = git(['show', `${ref}:schema/${version}/schema.json`], { cwd });
  try {
    return JSON.parse(text);
  } catch (err) {
    throw new Error(`schema/${version}/schema.json at ${ref} is not valid JSON: ${err.message}`);
  }
}

/**
 * The sections for every schema version that `head` adds or changes compared
 * to `base`. A new version is compared to the highest version below it; one
 * that also exists at `base` is compared to its own copy there.
 */
export function collectSections({ base, head, cwd }) {
  const atBase = versionsAt(base, cwd);
  const atHead = versionsAt(head, cwd);
  const all = [...new Set([...atBase.keys(), ...atHead.keys()])];

  const touched = [...atHead.entries()]
    .filter(([v, blob]) => atBase.get(v) !== blob)
    .map(([v]) => v)
    .sort(compareVersions);

  return touched.map((newVersion) => {
    const isNew = !atBase.has(newVersion);
    const oldVersion = isNew ? previousVersion(newVersion, all) : newVersion;
    const label = `schema/${newVersion}/schema.json`;
    if (!oldVersion) {
      return { newVersion, oldVersion: null, isNew, label, result: { changes: [], noise: [], diff: '' } };
    }
    // A released version is read from the base; one this PR also adds (two new
    // versions in one PR) only exists at the head.
    const oldRef = atBase.has(oldVersion) ? base : head;
    const result = compareSchemas({
      oldDoc: readSchema(oldRef, oldVersion, cwd),
      newDoc: readSchema(head, newVersion, cwd),
      oldVersion,
      newVersion,
      oldLabel: `schema/${oldVersion}/schema.json`,
      newLabel: label,
    });
    return { newVersion, oldVersion, isNew, label, result };
  });
}

// --- CLI ----------------------------------------------------------------------

function parseArgs(argv) {
  const args = { head: 'HEAD', maxChars: DEFAULT_MAX_CHARS };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    const value = argv[i + 1];
    if (flag === '--base') args.base = value;
    else if (flag === '--head') args.head = value;
    else if (flag === '--max-chars') args.maxChars = Number(value);
    else throw new Error(`unknown argument: ${flag}`);
    i++;
  }
  if (!args.base) throw new Error('--base <ref> is required');
  if (!Number.isInteger(args.maxChars) || args.maxChars <= 0) {
    throw new Error('--max-chars must be a positive integer');
  }
  return args;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const args = parseArgs(process.argv.slice(2));
    const sections = collectSections({ base: args.base, head: args.head, cwd: process.cwd() });
    process.stdout.write(renderComment(sections, { maxChars: args.maxChars }));
  } catch (err) {
    console.error(`schema-diff: ${err.message}`);
    process.exit(1);
  }
}
