// Finds tests that exist but that no CI job ever runs. Shared by
// check-orphan-tests.mjs and its test.
//
// A test that runs nowhere fails in no way at all: it stays green because it
// never starts. A test audit found seventeen of them, among them an engine test
// target no job named, a guard whose recipe only `just check` called, a loader
// test inside a BDD world and a `.test.sh` that only a Justfile recipe ran,
// which itself ran nowhere. This module reads what CI runs (the workflows, the
// pre-commit hooks, and every Justfile recipe and npm script they reach) and
// reports each test file outside it.
//
// It reads YAML, Justfiles and shell by line, without a parser: the Pre-commit
// job has Node but no node_modules. It leans towards counting a test as run. A
// recipe with a `case` counts as running every branch, `just a b` as running
// both recipes, and a file counts as run once it is the command or an argument
// of an interpreter (`node`, `bash`, `python3`, ...). What it cannot model is a
// condition (`if:` other than a literal `false`) or a trigger: a test in a
// workflow that only runs on a schedule, or a step with `continue-on-error`,
// still counts. Playwright `*.spec.*` files and pytest suites are out of scope.

import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, isAbsolute, join, posix, relative, resolve, sep } from 'node:path';

// --- Paths and globs --------------------------------------------------------

/** `abs` relative to `root`, with forward slashes; null when it lies outside. */
export function toRepo(root, abs) {
  const rel = relative(root, abs).split(sep).join('/');
  if (rel === '') return '';
  if (rel.startsWith('..') || isAbsolute(rel)) return null;
  return rel;
}

function escapeRegExp(text) {
  return text.replace(/[.+^${}()|[\]\\*?]/g, '\\$&');
}

/**
 * A glob as vitest and node take it: `**`, `*`, `?`, `{a,b}`, `?(a|b)` and
 * `[ab]`. Enough for the include and exclude lists in this repository.
 */
export function globToRegExp(glob) {
  const g = glob.replace(/^\.\//, '');
  let re = '';
  for (let i = 0; i < g.length; i += 1) {
    const c = g[i];
    if (c === '*' && g[i + 1] === '*') {
      if (g[i + 2] === '/') {
        re += '(?:[^/]*/)*';
        i += 2;
      } else {
        re += '.*';
        i += 1;
      }
    } else if (c === '*') {
      re += '[^/]*';
    } else if (c === '?' && g[i + 1] === '(') {
      const end = g.indexOf(')', i);
      re += `(?:${g.slice(i + 2, end).split('|').map(escapeRegExp).join('|')})?`;
      i = end;
    } else if (c === '?') {
      re += '[^/]';
    } else if (c === '{') {
      const end = g.indexOf('}', i);
      re += `(?:${g.slice(i + 1, end).split(',').map(escapeRegExp).join('|')})`;
      i = end;
    } else if (c === '[') {
      const end = g.indexOf(']', i);
      re += g.slice(i, end + 1);
      i = end;
    } else {
      re += escapeRegExp(c);
    }
  }
  return new RegExp(`^${re}$`);
}

// --- Vitest -----------------------------------------------------------------

const VITEST_CONFIGS = [
  'vitest.config.ts', 'vitest.config.mts', 'vitest.config.js', 'vitest.config.mjs',
  'vite.config.ts', 'vite.config.mts', 'vite.config.js', 'vite.config.mjs',
];
const VITEST_DEFAULT_INCLUDE = ['**/*.{test,spec}.?(c|m)[jt]s?(x)'];
// `configDefaults.exclude` since vitest 3.
const VITEST_DEFAULT_EXCLUDE = ['**/node_modules/**', '**/.git/**'];

/** `text` without its comments; strings stay, so a glob like `src/**` survives. */
function stripJsComments(text) {
  let out = '';
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (c === '"' || c === "'" || c === '`') {
      let j = i + 1;
      while (j < text.length && text[j] !== c) j += text[j] === '\\' ? 2 : 1;
      out += text.slice(i, j + 1);
      i = j;
    } else if (c === '/' && text[i + 1] === '/') {
      while (i < text.length && text[i] !== '\n') i += 1;
      out += '\n';
    } else if (c === '/' && text[i + 1] === '*') {
      const end = text.indexOf('*/', i + 2);
      i = end === -1 ? text.length : end + 1;
    } else {
      out += c;
    }
  }
  return out;
}

/** The text from the bracket at `open` to its partner, skipping strings. */
function balanced(text, open) {
  const pair = { '[': ']', '{': '}' }[text[open]];
  let depth = 0;
  for (let i = open; i < text.length; i += 1) {
    const c = text[i];
    if (c === '"' || c === "'" || c === '`') {
      i += 1;
      while (i < text.length && text[i] !== c) i += text[i] === '\\' ? 2 : 1;
    } else if (c === text[open]) {
      depth += 1;
    } else if (c === pair) {
      depth -= 1;
      if (depth === 0) return text.slice(open, i + 1);
    }
  }
  return text.slice(open);
}

/** The string literals of `key: [...]` at the top level of `block`. */
function stringList(block, key) {
  const re = new RegExp(`\\b${key}\\s*:\\s*\\[`, 'g');
  for (let m = re.exec(block); m; m = re.exec(block)) {
    // Only a key of this object, not of a nested one such as `coverage: {...}`.
    const before = block.slice(1, m.index);
    let depth = 0;
    for (const c of before) depth += c === '{' ? 1 : c === '}' ? -1 : 0;
    if (depth !== 0) continue;
    const list = balanced(block, m.index + m[0].length - 1);
    return [...list.matchAll(/(['"`])((?:(?!\1).)+)\1/g)].map((x) => x[2]);
  }
  return null;
}

/**
 * The include and exclude lists of the vitest config in `dir` (repo-relative),
 * read from its `test: {...}` block. No config, or no list, means vitest's own
 * default.
 */
const configCache = new Map();

export function vitestConfig(root, dir) {
  const key = join(root, dir);
  if (!configCache.has(key)) configCache.set(key, readVitestConfig(root, dir));
  return configCache.get(key);
}

function readVitestConfig(root, dir) {
  for (const name of VITEST_CONFIGS) {
    const file = join(root, dir, name);
    if (!existsSync(file)) continue;
    const text = stripJsComments(readFileSync(file, 'utf8'));
    const start = /\btest\s*:\s*\{/.exec(text);
    const block = start ? balanced(text, start.index + start[0].length - 1) : '';
    return {
      file: posix.join(dir, name),
      include: stringList(block, 'include') ?? VITEST_DEFAULT_INCLUDE,
      exclude: stringList(block, 'exclude') ?? VITEST_DEFAULT_EXCLUDE,
    };
  }
  return { file: null, include: VITEST_DEFAULT_INCLUDE, exclude: VITEST_DEFAULT_EXCLUDE };
}

/** Whether a vitest run rooted at `run.root` picks up `file`. */
export function vitestRuns(root, run, file) {
  const prefix = run.root === '' ? '' : `${run.root}/`;
  if (!file.startsWith(prefix)) return false;
  const rel = file.slice(prefix.length);
  const config = vitestConfig(root, run.root);
  if (!config.include.some((g) => globToRegExp(g).test(rel))) return false;
  if (config.exclude.some((g) => globToRegExp(g).test(rel))) return false;
  return run.filters.length === 0 || run.filters.some((f) => rel.includes(f) || file.includes(f));
}

// --- Reading YAML by line ---------------------------------------------------

function indentOf(line) {
  return line.length - line.trimStart().length;
}

function isBlankOrComment(line) {
  const t = line.trim();
  return t === '' || t.startsWith('#');
}

function unquote(value) {
  const v = value.trim();
  if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) {
    return v.slice(1, -1);
  }
  return v.replace(/\s+#.*$/, '');
}

/** The lines below `index` that are indented deeper than `indent`. */
function deeperLines(lines, index, indent) {
  const out = [];
  for (let i = index + 1; i < lines.length; i += 1) {
    const line = lines[i];
    if (line.trim() !== '' && indentOf(line) <= indent) break;
    out.push(line);
  }
  while (out.length && out[out.length - 1].trim() === '') out.pop();
  return out;
}

/** The value of `key:` on line `index`, inline or as a block below it. */
function scalarAt(lines, index, keyIndent, raw) {
  const value = raw.trim();
  const block = deeperLines(lines, index, keyIndent);
  const nonBlank = block.filter((l) => l.trim() !== '');
  const min = nonBlank.length ? Math.min(...nonBlank.map(indentOf)) : 0;
  const body = block.map((l) => l.slice(min));
  if (/^\|[-+]?\d*$/.test(value)) return body.join('\n');
  if (/^>[-+]?\d*$/.test(value)) return body.join(' ');
  if (value === '') return body.join(' ');
  return unquote(value);
}

/** The direct `key: value` children of the mapping that starts below `index`. */
function mappingBelow(lines, index, indent) {
  const children = {};
  const block = deeperLines(lines, index, indent);
  const own = block.filter((l) => !isBlankOrComment(l));
  if (!own.length) return children;
  const childIndent = indentOf(own[0]);
  for (const line of own) {
    if (indentOf(line) !== childIndent) continue;
    const match = /^\s*([A-Za-z0-9_.-]+):\s*(.*)$/.exec(line);
    if (match) children[match[1]] = unquote(match[2]);
  }
  return children;
}

/** Lines of the top-level `jobs:` entries, or the whole file as one job. */
function jobBlocks(lines) {
  const jobsAt = lines.findIndex((l) => /^jobs:\s*$/.test(l));
  if (jobsAt === -1) return [{ name: null, start: 0, end: lines.length }];
  const jobs = [];
  for (let i = jobsAt + 1; i < lines.length; i += 1) {
    if (/^\S/.test(lines[i]) && !isBlankOrComment(lines[i])) break;
    const match = /^ {2}([A-Za-z0-9_-]+):\s*(#.*)?$/.exec(lines[i]);
    if (match) {
      if (jobs.length) jobs[jobs.length - 1].end = i;
      jobs.push({ name: match[1], start: i, end: lines.length });
    }
  }
  return jobs;
}

/** Every value of `key:` in a job, which is where a matrix lists its legs. */
function matrixValues(lines, job, key) {
  const values = new Set();
  const re = new RegExp(`^\\s*(?:-\\s+)?${escapeRegExp(key)}:\\s*(.*)$`);
  for (let i = job.start; i < job.end; i += 1) {
    const match = re.exec(lines[i]);
    if (!match || insideKey(lines, i, job.start, 'exclude')) continue;
    const raw = match[1].trim();
    if (raw.startsWith('[')) {
      for (const v of raw.slice(1, raw.lastIndexOf(']')).split(',')) values.add(unquote(v));
    } else if (raw === '') {
      for (const l of deeperLines(lines, i, indentOf(lines[i]))) {
        const item = /^\s*-\s+(.*)$/.exec(l);
        if (item) values.add(unquote(item[1]));
      }
    } else {
      values.add(unquote(raw));
    }
  }
  return [...values].filter((v) => v !== '' && !v.includes('${{'));
}

/** Whether line `index` sits somewhere below a `key:` line. */
function insideKey(lines, index, start, key) {
  let indent = indentOf(lines[index]);
  for (let i = index - 1; i >= start && indent > 0; i -= 1) {
    if (isBlankOrComment(lines[i]) || indentOf(lines[i]) >= indent) continue;
    indent = indentOf(lines[i]);
    if (new RegExp(`^\\s*(-\\s+)?${key}:`).test(lines[i])) return true;
  }
  return false;
}

/** A literal `if: false`, which GitHub never runs. */
const NEVER = /^\s*(- )?if:\s*(false|\$\{\{\s*false\s*\}\})\s*$/;

/** `env:` and `defaults.run.working-directory` at one level of nesting. */
function levelSettings(lines, start, end, indent) {
  const settings = { env: {}, workingDirectory: null };
  for (let i = start; i < end; i += 1) {
    const line = lines[i];
    if (isBlankOrComment(line) || indentOf(line) !== indent) continue;
    if (/^\s*env:\s*$/.test(line)) Object.assign(settings.env, mappingBelow(lines, i, indent));
    if (/^\s*defaults:\s*$/.test(line)) {
      const runAt = lines.findIndex((l, j) => j > i && /^\s*run:\s*$/.test(l));
      if (runAt !== -1) {
        const wd = mappingBelow(lines, runAt, indentOf(lines[runAt]))['working-directory'];
        if (wd) settings.workingDirectory = wd;
      }
    }
  }
  return settings;
}

/**
 * Every `run:` step of a workflow or composite action, as a shell fragment:
 * its text, the directory it runs in and its environment. A step whose text
 * names `${{ matrix.x }}` yields one fragment per value of `x` in its job.
 */
export function workflowFragments(text, file) {
  const lines = text.split('\n');
  const fragments = [];
  const top = levelSettings(lines, 0, lines.length, 0);
  for (const job of jobBlocks(lines)) {
    if (job.name && lines.slice(job.start + 1, job.end).some((l) => indentOf(l) === 4 && NEVER.test(l))) continue;
    const jobLevel = job.name ? levelSettings(lines, job.start + 1, job.end, 4) : { env: {}, workingDirectory: null };
    for (let i = job.start; i < job.end; i += 1) {
      const match = /^(\s*)(- )?run:(.*)$/.exec(lines[i]);
      if (!match) continue;
      const keyIndent = match[1].length + (match[2] ? 2 : 0);
      const parent = lines.slice(job.start, i).reverse().find((l) => !isBlankOrComment(l) && indentOf(l) < keyIndent);
      if (parent && /^\s*defaults:\s*$/.test(parent)) continue;
      const run = scalarAt(lines, i, keyIndent, match[3]);
      // The step: from its `- ` to the next line at or above that indent.
      const dashIndent = keyIndent - 2;
      let stepStart = i;
      while (stepStart > job.start && !(indentOf(lines[stepStart]) === dashIndent && /^\s*- /.test(lines[stepStart]))) {
        stepStart -= 1;
      }
      let stepEnd = i + 1;
      while (stepEnd < job.end && (isBlankOrComment(lines[stepEnd]) || indentOf(lines[stepEnd]) > dashIndent)) stepEnd += 1;
      const step = { env: {}, workingDirectory: null, never: false };
      for (let j = stepStart; j < stepEnd; j += 1) {
        const line = lines[j].replace(/^(\s*)- /, '$1  ');
        if (indentOf(line) !== keyIndent) continue;
        if (NEVER.test(line)) step.never = true;
        const wd = /^\s*working-directory:\s*(.+)$/.exec(line);
        if (wd) step.workingDirectory = unquote(wd[1]);
        if (/^\s*env:\s*$/.test(line)) Object.assign(step.env, mappingBelow(lines, j, keyIndent));
      }
      if (step.never) continue;
      const fragment = {
        source: `${file}:${i + 1}`,
        text: run,
        cwd: step.workingDirectory ?? jobLevel.workingDirectory ?? top.workingDirectory ?? '',
        env: { ...top.env, ...jobLevel.env, ...step.env },
        persistentCd: true,
      };
      const matrixKeys = [...new Set([...run.matchAll(/\$\{\{\s*matrix\.([A-Za-z0-9_-]+)\s*\}\}/g)].map((m) => m[1]))];
      let variants = [fragment];
      for (const key of matrixKeys) {
        const values = matrixValues(lines, job, key);
        const re = new RegExp(`\\$\\{\\{\\s*matrix\\.${escapeRegExp(key)}\\s*\\}\\}`, 'g');
        variants = variants.flatMap((f) => values.map((v) => ({ ...f, text: f.text.replace(re, v) })));
      }
      fragments.push(...variants);
    }
  }
  return fragments;
}

/**
 * The local pre-commit hooks that run in CI, as shell fragments. The
 * Pre-commit job runs every hook with `--all-files`, so a hook runs there
 * unless its stages leave out `pre-commit` or its `files:` pattern matches no
 * file at all.
 */
export function precommitFragments(text, files, file = '.pre-commit-config.yaml') {
  const lines = text.split('\n');
  const fragments = [];
  for (let i = 0; i < lines.length; i += 1) {
    const match = /^(\s*)- id:\s*(\S+)/.exec(lines[i]);
    if (!match) continue;
    const dashIndent = match[1].length;
    const keys = {};
    for (let j = i + 1; j < lines.length; j += 1) {
      const line = lines[j];
      if (!isBlankOrComment(line) && indentOf(line) <= dashIndent) break;
      if (indentOf(line) !== dashIndent + 2) continue;
      const kv = /^\s*([a-z_]+):\s*(.*)$/.exec(line);
      if (kv) keys[kv[1]] = scalarAt(lines, j, dashIndent + 2, kv[2]);
    }
    if (!keys.entry) continue;
    if (keys.stages) {
      const stages = keys.stages.replace(/[[\]]/g, '').split(',').map((s) => s.trim());
      if (!stages.includes('pre-commit') && !stages.includes('commit')) continue;
    }
    if (keys.files || keys.exclude) {
      const re = new RegExp(keys.files ?? '');
      const ex = keys.exclude ? new RegExp(keys.exclude) : null;
      if (![...files].some((f) => re.test(f) && !(ex && ex.test(f)))) continue;
    }
    const args = keys.args ? keys.args.replace(/[[\]]/g, ' ').replace(/,/g, ' ') : '';
    fragments.push({
      source: `${file}:${i + 1} (${match[2]})`,
      text: `${keys.entry} ${args}`.trim(),
      cwd: '',
      env: {},
      persistentCd: true,
    });
  }
  return fragments;
}

// --- Justfile ---------------------------------------------------------------

/** Recipes (name, dependencies, body) and plain string variables. */
export function parseJustfile(text) {
  const lines = text.split('\n');
  const recipes = new Map();
  const vars = new Map();
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (/^\s|^#|^\[|^$/.test(line)) continue;
    const assign = /^([A-Za-z_][A-Za-z0-9_-]*)\s*:=\s*(.*)$/.exec(line);
    if (assign) {
      const value = /^(['"])(.*)\1\s*$/.exec(assign[2].trim());
      vars.set(assign[1], value ? value[2] : '');
      continue;
    }
    const recipe = /^@?([A-Za-z0-9_-]+)((?:\s+[^:]*)?):(?!=)(.*)$/.exec(line);
    if (!recipe || /^(set|mod|import|export|alias)$/.test(recipe[1])) continue;
    const deps = [];
    const depText = recipe[3].replace(/#.*$/, '');
    for (const m of depText.matchAll(/\(\s*([A-Za-z0-9_-]+)[^)]*\)|([A-Za-z0-9_-]+)/g)) deps.push(m[1] ?? m[2]);
    const body = [];
    let j = i + 1;
    for (; j < lines.length; j += 1) {
      if (lines[j].trim() !== '' && !/^\s/.test(lines[j])) break;
      body.push(lines[j].replace(/^ {4}|^\t/, ''));
    }
    recipes.set(recipe[1], {
      line: i + 1,
      deps,
      body: body.join('\n'),
      shebang: body.some((l) => l.startsWith('#!')),
    });
  }
  return { recipes, vars };
}

// --- Shell, by command ------------------------------------------------------

/** Replaces every `$(...)` with a placeholder, so that splitting on `|` keeps them whole. */
function protectSubstitutions(text) {
  const saved = [];
  let out = '';
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === '$' && text[i + 1] === '(') {
      let depth = 0;
      let j = i + 1;
      for (; j < text.length; j += 1) {
        if (text[j] === '(') depth += 1;
        if (text[j] === ')') depth -= 1;
        if (depth === 0) break;
      }
      saved.push(text.slice(i, j + 1));
      out += `\u0000${saved.length - 1}\u0000`;
      i = j;
    } else {
      out += text[i];
    }
  }
  return { text: out, restore: (s) => s.replace(/\u0000(\d+)\u0000/g, (_, n) => saved[Number(n)]) };
}

function stripShellComment(line) {
  if (/^\s*#/.test(line)) return '';
  return line.replace(/\s#.*$/, '');
}

/** Drops a subshell or group bracket around a command, but not the end of a `$(...)`. */
function unwrapGroup(command) {
  let c = command.trim().replace(/^[({]\s*/, '');
  const opens = (c.match(/\(/g) ?? []).length;
  const closes = (c.match(/\)/g) ?? []).length;
  if (closes > opens) c = c.replace(/\s*\)\s*$/, '');
  return c.replace(/\s*\}\s*$/, '');
}

/** Commands whose arguments are files they run. */
const RUNNERS = new Set(['node', 'bash', 'sh', 'zsh', 'python', 'python3', 'pytest', 'deno', 'bun', 'tsx', 'source', '.']);

function tokens(command) {
  return command.replace(/["']/g, '').split(/\s+/).filter(Boolean);
}

/**
 * Follows everything CI runs, from the workflow steps and hooks down through
 * Justfile recipes and npm scripts, and records what it meets: files named in
 * a command, vitest runs, and cargo test runs.
 */
export class CiModel {
  constructor(root, files) {
    this.root = root;
    this.files = new Set(files);
    const justfile = ['Justfile', 'justfile'].map((n) => join(root, n)).find((p) => existsSync(p));
    this.just = justfile ? parseJustfile(readFileSync(justfile, 'utf8')) : { recipes: new Map(), vars: new Map() };
    this.mentions = new Map();
    this.vitest = [];
    this.cargo = [];
    this.seen = new Set();
    // The environment of the command being followed, which a recipe, npm
    // script or shell script it starts inherits.
    this.env = {};
  }

  abs(cwd) {
    return resolve(this.root, cwd);
  }

  /**
   * Records each word that is a file in the repository, seen from `cwd`. With
   * `dirs`, a directory counts for the test files below it, as `node --test
   * dir/` runs them.
   */
  mention(words, cwd, source, dirs = false) {
    for (const word of words) {
      if (!/[./]/.test(word) || word.startsWith('-')) continue;
      if (word.includes('*')) {
        const base = toRepo(this.root, this.abs(cwd));
        const re = globToRegExp(posix.join(base ?? '', word));
        for (const f of this.files) if (re.test(f) && !this.mentions.has(f)) this.mentions.set(f, source);
        continue;
      }
      const path = toRepo(this.root, resolve(this.abs(cwd), word));
      if (dirs && path !== null) {
        for (const f of this.files) {
          if (f.startsWith(path === '' ? '' : `${path}/`) && /\.test\.[cm]?[jt]s$/.test(f) && !this.mentions.has(f)) {
            this.mentions.set(f, source);
          }
        }
      }
      if (path && this.files.has(path)) {
        if (!this.mentions.has(path)) this.mentions.set(path, source);
        // A shell script CI runs counts with what it runs in turn.
        if (path.endsWith('.sh')) this.script(path, source);
      }
    }
  }

  script(path, source) {
    const key = `sh:${path}:${JSON.stringify(this.env)}`;
    if (this.seen.has(key)) return;
    this.seen.add(key);
    const text = readFileSync(join(this.root, path), 'utf8')
      .replace(/\$\(dirname\s+"?\$0"?\)|\$\{0%\/\*\}|\$\{BASH_SOURCE\[0\]%\/\*\}/g, dirname(join(this.root, path)));
    this.fragment({ source, text, cwd: '', env: { ...this.env }, persistentCd: true });
  }

  fragment({ source, text, cwd, env, persistentCd }) {
    const ROOT = this.root;
    let shellCwd = cwd;
    // What a command started from here sees (`exported`), and plain shell
    // variables, which only `$NAME` in this shell sees. Like the directory,
    // both reset per line in a recipe whose lines each get their own shell.
    const freshShell = () => ({ exported: { ...env }, vars: {} });
    let shell = freshShell();
    const joined = text
      .replace(/\\\n/g, ' ')
      .replace(/\$\{\{\s*github\.workspace\s*\}\}|\$\{GITHUB_WORKSPACE\}|\$GITHUB_WORKSPACE/g, ROOT);
    for (const rawLine of joined.split('\n')) {
      const line = stripShellComment(rawLine);
      if (line.trim() === '' || line.startsWith('#!')) continue;
      let lineCwd = shellCwd;
      if (!persistentCd) shell = freshShell();
      const prot = protectSubstitutions(line);
      for (const piece of prot.text.split(/&&|\|\||;|\||\bthen\b|\bdo\b|\belse\b/)) {
        const command = unwrapGroup(prot.restore(piece))
          .replace(/\$\(pwd\)|\$\{PWD\}|\$PWD/g, this.abs(lineCwd));
        lineCwd = this.command(command, lineCwd, shell, source, text);
      }
      if (persistentCd) shellCwd = lineCwd;
    }
  }

  /** Runs one simple command; returns the directory the next one starts in. */
  command(rawCommand, cwd, shell, source, fragmentText) {
    // What a `$(...)` runs, runs too.
    for (const inner of rawCommand.matchAll(/\$\(([^()]*(?:\([^()]*\)[^()]*)*)\)/g)) {
      const sub = { exported: { ...shell.exported }, vars: { ...shell.vars } };
      for (const part of inner[1].split(/&&|\|\||;|\|/)) this.command(part, cwd, sub, source, fragmentText);
    }
    let command = rawCommand.trim();
    let exporting = false;
    // Keywords and wrappers in front of the command that runs.
    for (;;) {
      if (/^export\s+/.test(command)) exporting = true;
      const prefix = /^(if|elif|while|until|!|export|exec|time|command|sudo|then|do)\s+/.exec(command)
        ?? /^(timeout)\s+(-\S+\s+)*\S+\s+/.exec(command)
        ?? /^(env|xargs|npx)\s+(-\S+\s+)*/.exec(command);
      if (!prefix || prefix[0] === '') break;
      command = command.slice(prefix[0].length);
    }
    // `VAR=value` on its own sets a shell variable for what follows.
    // The value is pinned where it is set: variables it names are filled in,
    // and a script it substitutes gets an absolute path, so a recipe that
    // inherits it still finds that script.
    const ASSIGN = /^([A-Za-z_][A-Za-z0-9_]*)=(\$\(.*\)|"[^"]*"|'[^']*'|\S*)$/;
    const assignments = exporting
      ? command.trim().split(/\s+(?=[A-Za-z_][A-Za-z0-9_]*=)/).map((part) => ASSIGN.exec(part))
      : [ASSIGN.exec(command.trim())];
    if (assignments.every(Boolean)) {
      for (const [, name, raw] of assignments) {
        const known = { ...shell.vars, ...shell.exported };
        const value = raw.startsWith("'")
          ? raw.slice(1, -1)
          : raw
              .replace(/^"|"$/g, '')
              // `${NAME:-default}` and other operators stay as they are.
              .replace(/\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)/g, (m, a, b) => known[a ?? b] ?? m)
              .replace(/\$\(\s*([^\s)]+\.sh)\s*\)/g, (_, p) => `$(${resolve(this.abs(cwd), p)})`);
        if (exporting || name in shell.exported) shell.exported[name] = value;
        else shell.vars[name] = value;
      }
      return cwd;
    }
    // `export NAME` hands an existing shell variable to what follows.
    if (exporting && /^[A-Za-z_][A-Za-z0-9_]*(\s+[A-Za-z_][A-Za-z0-9_]*)*$/.test(command.trim())) {
      for (const name of command.trim().split(/\s+/)) {
        if (name in shell.vars) shell.exported[name] = shell.vars[name];
      }
      return cwd;
    }
    let words = tokens(command);
    const inline = {};
    while (words.length && /^[A-Za-z_][A-Za-z0-9_]*=/.test(words[0])) {
      const [name, ...rest] = words.shift().split('=');
      inline[name] = rest.join('=');
    }
    while (['env', 'xargs', 'npx'].includes(words[0])) {
      words = words.slice(1);
      while (words[0]?.startsWith('-')) words = words.slice(1);
    }
    if (!words.length) return cwd;
    const [cmd, ...args] = words;
    const allEnv = { ...shell.exported, ...inline };
    const outer = this.env;
    this.env = allEnv;
    try {
      return this.dispatch(cmd, args, cwd, allEnv, source, fragmentText);
    } finally {
      this.env = outer;
    }
  }

  dispatch(cmd, args, cwd, allEnv, source, fragmentText) {
    // A file counts as run when it is the command, or an argument of an
    // interpreter or test runner. `echo` or `git diff` naming it does not run it.
    this.mention([cmd], cwd, source);
    if (RUNNERS.has(cmd)) this.mention(args, cwd, source, cmd === 'node' && args.includes('--test'));
    if (cmd === 'cd' && args[0]) {
      const target = toRepo(this.root, resolve(this.abs(cwd), args[0]));
      return target ?? cwd;
    }
    if (cmd === 'just') this.justCall(args, source);
    else if (cmd === 'npm') this.npmCall(args, cwd, source);
    else if (cmd === 'vitest') this.vitestCall(args, cwd, source);
    else if (cmd === 'cargo' && args[0] === 'test') this.cargoCall(args.slice(1), cwd, allEnv, source, fragmentText);
    return cwd;
  }

  justCall(args, source) {
    // `just a b` runs both when `a` takes no arguments; following every word
    // that names a recipe errs towards counting a run.
    for (const name of args.filter((a) => !a.includes('=') && !a.startsWith('-'))) this.recipe(name, source);
  }

  recipe(name, source) {
    const recipe = this.just.recipes.get(name);
    const key = `just:${name}:${JSON.stringify(this.env)}`;
    if (!recipe || this.seen.has(key)) return;
    this.seen.add(key);
    for (const dep of recipe.deps) this.recipe(dep, source);
    const body = recipe.body.replace(/\{\{\s*([^}]*?)\s*\}\}/g, (_, expr) => {
      if (/^justfile_directory\(\)$/.test(expr)) return this.root;
      return this.just.vars.get(expr) ?? '';
    });
    this.fragment({
      source: `${source} → just ${name}`,
      text: body,
      cwd: '',
      env: { ...this.env },
      persistentCd: recipe.shebang,
    });
  }

  npmCall(args, cwd, source) {
    const dirs = [];
    let script = null;
    for (let i = 0; i < args.length; i += 1) {
      const a = args[i];
      if (a === '-w' || a === '--workspace' || a === '--prefix') {
        dirs.push(args[i + 1]);
        i += 1;
      } else if (a.startsWith('--workspace=') || a.startsWith('--prefix=')) {
        dirs.push(a.split('=')[1]);
      } else if (a === '--') {
        break;
      } else if (!a.startsWith('-') && script === null) {
        if (a === 'test' || a === 't' || a === 'tst') script = 'test';
        else if (a === 'run' || a === 'run-script') script = '';
        else if (script === null) return;
      } else if (!a.startsWith('-') && script === '') {
        script = a;
      }
    }
    if (!script) return;
    const targets = dirs.length ? dirs : ['.'];
    for (const dir of targets) {
      const pkgDir = toRepo(this.root, resolve(this.abs(cwd), dir));
      if (pkgDir === null) continue;
      this.npmScript(pkgDir, script, source);
    }
  }

  npmScript(pkgDir, script, source) {
    const key = `npm:${pkgDir}#${script}:${JSON.stringify(this.env)}`;
    if (this.seen.has(key)) return;
    this.seen.add(key);
    const file = join(this.root, pkgDir, 'package.json');
    if (!existsSync(file)) return;
    const scripts = JSON.parse(readFileSync(file, 'utf8')).scripts ?? {};
    for (const name of [`pre${script}`, script, `post${script}`]) {
      if (!scripts[name]) continue;
      this.fragment({
        source: `${source} → npm run ${name} (${pkgDir || '.'})`,
        text: scripts[name],
        cwd: pkgDir,
        env: { ...this.env },
        persistentCd: true,
      });
    }
  }

  vitestCall(args, cwd, source) {
    let rootArg = null;
    const filters = [];
    for (let i = 0; i < args.length; i += 1) {
      const a = args[i];
      if (a === '--root' || a === '-r' || a === '--dir') {
        rootArg = args[i + 1];
        i += 1;
      } else if (a.startsWith('--root=') || a.startsWith('--dir=')) {
        rootArg = a.split('=')[1];
      } else if (a === '--config' || a === '-c' || a === '--project' || a === '--reporter') {
        i += 1;
      } else if (!a.startsWith('-') && !['run', 'watch', 'related'].includes(a)) {
        filters.push(a);
      }
    }
    const root = toRepo(this.root, resolve(this.abs(cwd), rootArg ?? '.'));
    if (root !== null) this.vitest.push({ root, filters, source });
  }

  cargoCall(args, cwd, env, source, fragmentText) {
    const run = { cwd, workspace: false, packages: [], excludes: [], targets: [], partial: false, env, source, fragmentText };
    for (let i = 0; i < args.length; i += 1) {
      const a = args[i];
      if (a === '--') break;
      // Builds the tests without running them.
      if (a === '--no-run') return;
      if (a === '--workspace' || a === '--all') run.workspace = true;
      else if (a === '-p' || a === '--package') run.packages.push(args[++i]);
      else if (a.startsWith('-p') && a.length > 2) run.packages.push(a.slice(2));
      else if (a.startsWith('--package=')) run.packages.push(a.split('=')[1]);
      else if (a === '--exclude') run.excludes.push(args[++i]);
      else if (a.startsWith('--exclude=')) run.excludes.push(a.split('=')[1]);
      else if (a === '--test') run.targets.push(args[++i]);
      else if (a.startsWith('--test=')) run.targets.push(a.split('=')[1]);
      else if (a === '--manifest-path') run.cwd = toRepo(this.root, dirname(resolve(this.abs(cwd), args[++i]))) ?? cwd;
      else if (['--lib', '--bins', '--bin', '--doc', '--examples', '--example', '--benches', '--bench'].includes(a)) {
        run.partial = true;
        if (['--bin', '--example', '--bench'].includes(a)) i += 1;
      }
    }
    if (run.targets.length) run.partial = true;
    this.cargo.push(run);
  }
}

/** Builds the model from the workflows, the composite actions and the hooks. */
export function readCi(root, files) {
  const model = new CiModel(root, files);
  const yamls = [...files].filter((f) => /^\.github\/(workflows\/[^/]+|actions\/.+\/action)\.ya?ml$/.test(f));
  for (const file of yamls) {
    for (const fragment of workflowFragments(readFileSync(join(root, file), 'utf8'), file)) model.fragment(fragment);
  }
  const hooks = join(root, '.pre-commit-config.yaml');
  if (existsSync(hooks)) {
    for (const fragment of precommitFragments(readFileSync(hooks, 'utf8'), files)) model.fragment(fragment);
  }
  return model;
}

// --- Cargo ------------------------------------------------------------------

/** Workspace members of `packages/Cargo.toml`-style manifests, with their crate names. */
export function cargoWorkspace(root, files) {
  const manifests = [...files].filter((f) => f.endsWith('Cargo.toml'));
  for (const manifest of manifests) {
    const text = readFileSync(join(root, manifest), 'utf8');
    if (!/^\[workspace\]/m.test(text)) continue;
    const block = /^members\s*=\s*\[([^\]]*)\]/m.exec(text);
    if (!block) continue;
    const base = posix.dirname(manifest);
    const members = [...block[1].replace(/#.*$/gm, '').matchAll(/"([^"]+)"/g)].flatMap((m) => {
      const member = posix.join(base, m[1]);
      if (!member.includes('*')) return [member];
      const re = globToRegExp(`${member}/Cargo.toml`);
      return [...files].filter((f) => re.test(f)).map((f) => posix.dirname(f));
    });
    return {
      dir: base === '.' ? '' : base,
      crates: members.map((dir) => {
        const own = readFileSync(join(root, dir, 'Cargo.toml'), 'utf8');
        const name = /^\[package\][^[]*?^name\s*=\s*"([^"]+)"/m.exec(own)?.[1] ?? posix.basename(dir);
        return { dir, name, tests: testTargets(own, dir) };
      }),
    };
  }
  return { dir: null, crates: [] };
}

/** The `[[test]]` targets a manifest declares, with `test = false` noted. */
function testTargets(text, dir) {
  const targets = [];
  for (const block of text.split(/^\[\[test\]\]\s*$/m).slice(1)) {
    const own = block.split(/^\[/m)[0];
    const name = /^name\s*=\s*"([^"]+)"/m.exec(own)?.[1];
    if (!name) continue;
    const path = /^path\s*=\s*"([^"]+)"/m.exec(own)?.[1];
    targets.push({
      name,
      paths: path ? [posix.join(dir, path)] : [posix.join(dir, 'tests', `${name}.rs`), posix.join(dir, 'tests', name, 'main.rs')],
      skipped: /^test\s*=\s*false/m.test(own),
    });
  }
  return targets;
}

/** Which crates a cargo run covers. */
function cratesOf(run, workspace) {
  const atRoot = run.cwd === workspace.dir;
  const member = workspace.crates.find((c) => run.cwd === c.dir || run.cwd.startsWith(`${c.dir}/`));
  if (run.packages.length) return workspace.crates.filter((c) => run.packages.includes(c.name));
  if (run.workspace || (atRoot && !member)) {
    return workspace.crates.filter((c) => !run.excludes.includes(c.name));
  }
  return member ? [member] : [];
}

// --- The rules --------------------------------------------------------------

/**
 * BDD runners outside the engine. Each names its runner (the test that reads
 * the feature files) and the directory it reads them from; the guard checks
 * that the runner itself runs in CI. The directory is a claim about the
 * runner's code: each runner fails loudly when its directory is missing.
 */
export const BDD_RUNNERS = [
  // `corpus-poc/napp/scenarios`, joined in main.rs.
  { runner: 'packages/poc-napp/tests/bdd/main.rs', root: 'corpus-poc/napp/scenarios' },
  // `corpusDir` in tests/helpers/casusPaden.js.
  { runner: 'frontend-poc-terugbetaalregimes/tests/scenarios.test.js', root: 'corpus-poc/terugbetaalregimes/corpus' },
  { runner: 'frontend-poc-nieuwkomersbekostiging/tests/scenarios.test.js', root: 'corpus-poc/nieuwkomersbekostiging/corpus' },
];

/** The engine's own runner, configured per run by BDD_BUCKET and REGULATION_PATH. */
export const ENGINE_BDD = {
  crate: 'regelrecht-engine',
  target: 'bdd',
  // bucket B
  conformance: 'bdd/conformance',
  // bucket A without REGULATION_PATH (default_regulation_base_path)
  defaultCorpus: 'corpus/regulation',
};

/**
 * Paths the guard leaves alone, each with the reason. An exception nobody
 * needs any more is itself a finding.
 */
export const EXCEPTIONS = [
  {
    rule: 'feature',
    path: 'corpus/regulation/',
    reason:
      'Bucket A over the live laws stays out of CI on purpose: it asserts what the laws produce today, ' +
      'so red there is a question for a human (packages/engine/tests/bdd/main.rs, "Bucket selection").',
  },
];

function under(path, dir) {
  return dir === '' || path === dir || path.startsWith(`${dir}/`);
}

/** The repo-relative directories REGULATION_PATH names in one cargo run. */
function regulationDirs(root, run) {
  const value = run.env.REGULATION_PATH?.replace(/\$\{\{\s*github\.workspace\s*\}\}|\$\{GITHUB_WORKSPACE\}|\$GITHUB_WORKSPACE/g, root);
  if (value === undefined || value === '') return [ENGINE_BDD.defaultCorpus];
  if (!value.includes('$')) {
    return value.split(':').map((p) => toRepo(root, resolve(root, run.cwd, p))).filter((p) => p !== null);
  }
  // Built by a script in the same step, as for the chronolexografie PoC: run
  // the scripts the step substitutes and take their lines as directories.
  const dirs = [];
  for (const m of `${value} ${run.fragmentText}`.matchAll(/\$\(\s*([^\s)]+\.sh)\s*\)/g)) {
    const script = resolve(root, run.cwd, m[1]);
    if (!existsSync(script)) continue;
    const out = spawnSync('bash', [script], { cwd: resolve(root, run.cwd), encoding: 'utf8' });
    if (out.status !== 0) continue;
    for (const line of out.stdout.split('\n')) {
      const dir = line.trim() && toRepo(root, resolve(root, line.trim()));
      if (dir) dirs.push(dir);
    }
  }
  return dirs;
}

function hasReasonAbove(lines, index, isAttribute, isComment) {
  for (let i = index - 1; i >= 0; i -= 1) {
    const line = lines[i].trim();
    if (line === '') return false;
    if (isAttribute(line)) continue;
    return isComment(line);
  }
  return false;
}

/**
 * Every finding: tests CI never runs, crates it never tests, feature files no
 * runner reads, and skips without a reason.
 */
export function findOrphans(root, fileList, options = {}) {
  // Tracked but deleted from the working tree: tests that are about to go.
  const files = fileList.filter((f) => !/\.(test\.[a-z]+|feature|rs)$/.test(f) || existsSync(join(root, f)));
  const runners = options.runners ?? BDD_RUNNERS;
  const engine = options.engine ?? ENGINE_BDD;
  const exceptions = (options.exceptions ?? EXCEPTIONS).map((e) => ({ ...e, used: false }));
  const ci = readCi(root, files);
  const findings = [];
  const excepted = (rule, file) => {
    const hit = exceptions.find((e) => e.rule === rule && (file === e.path || file.startsWith(e.path)));
    if (hit) hit.used = true;
    return Boolean(hit);
  };

  // A JS test runs when a CI command names it, or a vitest run picks it up.
  const jsRuns = (file) => ci.mentions.has(file) || ci.vitest.some((run) => vitestRuns(root, run, file));

  // 1. JS tests.
  for (const file of files.filter((f) => /\.test\.(ts|js|mjs)$/.test(f))) {
    if (jsRuns(file) || excepted('js', file)) continue;
    findings.push({
      rule: 'js',
      file,
      message: 'no CI step, hook, recipe or npm script runs this test, and no vitest run in CI includes it',
    });
  }

  // 2. Shell tests.
  for (const file of files.filter((f) => /(^|\/)(test-[^/]*|[^/]*\.test)\.sh$/.test(f))) {
    if (ci.mentions.has(file) || excepted('sh', file)) continue;
    findings.push({
      rule: 'sh',
      file,
      message: 'no pre-commit hook or workflow step runs this script, directly or through a recipe they call',
    });
  }

  // Crates: every workspace member is tested by some cargo run in CI.
  const workspace = cargoWorkspace(root, files);
  const fullyTested = new Set();
  const targetsRun = new Set();
  for (const run of ci.cargo) {
    const crates = cratesOf(run, workspace);
    for (const c of crates) {
      if (!run.partial) fullyTested.add(c.name);
      for (const t of run.targets) targetsRun.add(`${c.name}:${t}`);
    }
  }
  for (const c of workspace.crates) {
    if (fullyTested.has(c.name) || excepted('crate', c.dir)) continue;
    findings.push({ rule: 'crate', file: `${c.dir}/Cargo.toml`, message: `no CI job runs \`cargo test\` over ${c.name}` });
  }
  const rustTargetRuns = (path) => {
    const crate = workspace.crates.find((c) => under(path, c.dir));
    if (!crate) return false;
    const target = crate.tests.find((t) => t.paths.includes(path));
    if (target && targetsRun.has(`${crate.name}:${target.name}`)) return true;
    return fullyTested.has(crate.name) && !(target && target.skipped);
  };

  // 3. Feature files: under a directory a runner in CI reads.
  const roots = [];
  for (const run of ci.cargo) {
    if (!run.targets.includes(engine.target)) continue;
    if (!cratesOf(run, workspace).some((c) => c.name === engine.crate)) continue;
    const bucket = (run.env.BDD_BUCKET ?? 'all').trim() || 'all';
    if (bucket === 'all' || bucket === 'conformance') roots.push({ dir: engine.conformance, scenariosOnly: false });
    if (bucket === 'all' || bucket === 'corpus') {
      for (const dir of regulationDirs(root, run)) roots.push({ dir, scenariosOnly: true });
    }
  }
  for (const { runner, root: dir } of runners) {
    const runs = runner.endsWith('.rs') ? rustTargetRuns(runner) : jsRuns(runner);
    if (runs) roots.push({ dir, scenariosOnly: false });
  }
  for (const file of files.filter((f) => f.endsWith('.feature'))) {
    const covered = roots.some(({ dir, scenariosOnly }) => {
      if (!under(file, dir)) return false;
      return !scenariosOnly || file.slice(dir.length).split('/').includes('scenarios');
    });
    if (covered || excepted('feature', file)) continue;
    findings.push({ rule: 'feature', file, message: 'no BDD runner that runs in CI reads this feature file' });
  }

  // 4. Skips without a reason.
  for (const file of files.filter((f) => f.endsWith('.feature'))) {
    const lines = readFileSync(join(root, file), 'utf8').split('\n');
    lines.forEach((line, i) => {
      const t = line.trim();
      if (!t.startsWith('@') || !/(^|\s)@wip(\s|$)/.test(t.replace(/#.*$/, ''))) return;
      if (/#\s*\S/.test(t)) return;
      if (hasReasonAbove(lines, i, (l) => l.startsWith('@'), (l) => /^#\s*\S/.test(l))) return;
      findings.push({ rule: 'wip', file, line: i + 1, message: '@wip without a comment that says why (above it or on the same line)' });
    });
  }
  for (const file of files.filter((f) => f.endsWith('.rs'))) {
    const lines = readFileSync(join(root, file), 'utf8').split('\n');
    lines.forEach((line, i) => {
      const t = line.trim();
      if (!/^#\[ignore\b/.test(t)) return;
      if (/^#\[ignore\s*=\s*"[^"]*\S[^"]*"\s*\]/.test(t)) return;
      if (/\]\s*\/\/\s*\S/.test(t)) return;
      if (hasReasonAbove(lines, i, (l) => l.startsWith('#['), (l) => /^\/\/[/!]?\s*\S/.test(l))) return;
      findings.push({ rule: 'ignore', file, line: i + 1, message: '#[ignore] without a reason (`#[ignore = "..."]` or a comment above it)' });
    });
  }

  for (const e of exceptions.filter((x) => !x.used)) {
    findings.push({ rule: 'exception', file: e.path, message: `exception for rule "${e.rule}" matches nothing any more; remove it` });
  }
  return findings;
}

/** Tracked files (staged ones included), as the guard sees the repository. */
export function trackedFiles(root) {
  const list = (...args) =>
    execFileSync('git', ['-C', root, 'ls-files', '-z', ...args], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 })
      .split('\u0000')
      .filter(Boolean);
  // Not `--deleted`: that stats every file, which costs seconds on a slow
  // filesystem. A file deleted but not yet staged is filtered where it is read.
  return list('--cached');
}
