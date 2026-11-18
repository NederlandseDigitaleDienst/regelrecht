/**
 * Every demo law has an executing organisation in services.yaml.
 *
 * The organisation used to sit in the law file itself, which meant a new law
 * carried its own answer. Now it lives in `services.yaml` under `laws:`, and
 * the app counts on every law having one: logo, colour and group heading come
 * from it. This check is what replaces the guarantee the old field gave for
 * free; copy-demo-corpus.mjs refuses such a law as well, as a last stop before
 * the data reaches the app.
 *
 * It also holds the map to real organisations, and refuses an entry for a law
 * that no longer exists, so the map cannot rot in either direction.
 *
 * Reads the two files line by line rather than with a YAML parser, the way the
 * other corpus guards do (`script/awb-parity.test.mjs`): it runs as a
 * pre-commit hook, in a CI job that installs no frontend dependencies. The
 * shapes it needs are flat and it insists on them, so a file that stops
 * matching them fails the check instead of being misread.
 */
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const corpusDir = resolve(here, '..', '..', 'corpus', 'demo');
const lawsDir = join(corpusDir, 'regulation', 'nl');
const servicesFile = join(corpusDir, 'services.yaml');

function walk(dir, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (entry.endsWith('.yaml')) out.push(full);
  }
  return out;
}

/**
 * The organisation codes under `services:` and the law map under `laws:`.
 * Both are two-space-indented keys under a top-level block, so one pass with
 * a note of which block we are in is enough.
 */
function readServices(text) {
  const organisations = new Set();
  const map = new Map();
  let block = null;
  for (const [i, raw] of text.split('\n').entries()) {
    const line = raw.replace(/\s+$/, '');
    if (!line || line.startsWith('#') || line === '---') continue;
    if (/^services:\s*$/.test(line)) { block = 'services'; continue; }
    if (/^laws:\s*$/.test(line)) { block = 'laws'; continue; }
    if (/^\S/.test(line)) { block = null; continue; }
    if (block === 'services') {
      const m = /^ {2}([A-Za-z0-9_]+):\s*$/.exec(line);
      if (m) organisations.add(m[1]);
      continue;
    }
    if (block === 'laws') {
      const m = /^ {2}(\S+):\s+(\S+)\s*$/.exec(line);
      if (!m) {
        console.error(`services.yaml regel ${i + 1} past niet op "  <wet>: <organisatie>": ${line}`);
        process.exit(1);
      }
      map.set(m[1], m[2]);
    }
  }
  return { organisations, map };
}

const { organisations, map } = readServices(readFileSync(servicesFile, 'utf8'));

/**
 * A law's `$id`. Two files carry one too long for a line and fold it
 * (`$id: >-` with the value indented underneath), so both forms count.
 */
function readId(lines) {
  const i = lines.findIndex((l) => /^\$id:/.test(l));
  if (i === -1) return null;
  const inline = lines[i].replace(/^\$id:\s*/, '').trim();
  if (inline && inline !== '>-' && inline !== '>' && inline !== '|') return inline;
  const folded = lines[i + 1];
  return folded && /^\s+\S/.test(folded) ? folded.trim() : null;
}

const ids = new Set();
const carriesField = [];
for (const file of walk(lawsDir)) {
  const lines = readFileSync(file, 'utf8').split('\n');
  const id = readId(lines);
  if (!id) continue;
  ids.add(id);
  if (lines.some((l) => /^service:\s/.test(l))) carriesField.push(file);
}

const problems = [];
for (const id of [...ids].sort()) {
  if (!map.has(id)) problems.push(`wet zonder organisatie in services.yaml: ${id}`);
}
for (const [id, code] of map) {
  if (!ids.has(id)) problems.push(`services.yaml noemt een wet die niet bestaat: ${id}`);
  if (!organisations.has(code)) problems.push(`${id} verwijst naar onbekende organisatie ${code}`);
}
for (const file of carriesField) {
  problems.push(`wetsbestand draagt nog een service-veld: ${file}`);
}

if (problems.length) {
  console.error('services.yaml en het democorpus lopen uiteen:\n');
  for (const p of problems) console.error('  ' + p);
  console.error(`\n${problems.length} probleem(en).`);
  process.exit(1);
}
console.log(`services.yaml dekt alle ${ids.size} demowetten, met ${organisations.size} organisaties.`);
