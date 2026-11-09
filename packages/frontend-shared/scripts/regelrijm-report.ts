// Report per corpus how RegelRijm reads it: actions, how many read on one
// line, the longest line, names without a term, and whether every action
// reads back to its tree.
//
//   npm run regelrijm-report -w packages/frontend-shared -- <dir>… [--glossary file.yaml] [--names]
//
// Paths are relative to where you run the command, also through npm, which
// runs the script in packages/frontend-shared but says where it started in
// INIT_CWD.
//
// A glossary is a YAML map `name: term`. It is read, never written, so a
// corpus kept elsewhere can be measured with its own words without them entering
// this repository.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import { load } from 'js-yaml';
import { articleContext, normaliseAction, renderAction, untranslated } from '../src/regelrijm/index';
import { corpusActions } from '../src/regelrijm/test-support/corpus';
import { parseActionAll } from '../src/regelrijm/test-support/parse';
import { isRecord, type Glossary } from '../src/regelrijm/types';

const USAGE = 'usage: regelrijm-report.ts <dir>… [--glossary glossary.yaml] [--names]';
const fromCwd = (p: string) => resolve(process.env.INIT_CWD ?? process.cwd(), p);
const args = process.argv.slice(2);
const gi = args.indexOf('--glossary');
const glossaryArg = gi >= 0 ? args.splice(gi, 2)[1] : undefined;
if (gi >= 0 && !glossaryArg) {
  console.error(USAGE);
  process.exit(2);
}
const glossaryFile = glossaryArg ? fromCwd(glossaryArg) : undefined;
const glossaryDoc: unknown = glossaryFile ? load(readFileSync(glossaryFile, 'utf8')) : undefined;
if (glossaryFile && !isRecord(glossaryDoc)) {
  console.error(`${glossaryFile}: a glossary is a YAML map \`name: term\``);
  process.exit(2);
}
const glossary = glossaryDoc as Glossary | undefined;
const ni = args.indexOf('--names');
const showNames = ni >= 0;
if (showNames) args.splice(ni, 1);
if (!args.length) {
  console.error(USAGE);
  process.exit(2);
}

const plain = (v: unknown): unknown => JSON.parse(JSON.stringify(v));

for (const dir of args) {
  const actions = corpusActions(fromCwd(dir));
  let oneLine = 0;
  let withUntranslated = 0;
  let roundTrip = 0;
  let longest = { length: 0, where: '' };
  const names = new Map<string, number>();
  let nameTokens = 0;
  for (const { file, article, action } of actions) {
    const ctx = articleContext(article, glossary);
    const r = renderAction(action, { article, ctx });
    if (!r.text.includes('\n')) oneLine += 1;
    for (const l of r.text.split('\n')) {
      const len = [...l.replace(/\t/g, '    ')].length;
      if (len > longest.length) longest = { length: len, where: `${file} ${isRecord(action) ? action.output : ''}` };
    }
    nameTokens += r.segments.filter((t) => typeof t !== 'string' && (t.tk === 'name' || t.tk === 'term')).length;
    const u = untranslated(r.segments);
    if (u.size) withUntranslated += 1;
    for (const [n, c] of u) names.set(n, (names.get(n) ?? 0) + c);
    const parses = parseActionAll(r.text, ctx);
    const want = normaliseAction(action);
    delete want.resolve;
    if (parses.length === 1 && isDeepStrictEqual(plain(parses[0]), plain(want))) roundTrip += 1;
  }
  const occurrences = [...names.values()].reduce((a, b) => a + b, 0);
  const pct = (n: number) => (actions.length ? `${Math.round((100 * n) / actions.length)}%` : '-');
  console.log(`\n${dir}`);
  console.log(`  actions                     ${actions.length}`);
  console.log(`  round trip exact            ${roundTrip} (${pct(roundTrip)})`);
  console.log(`  on one line                 ${oneLine} (${pct(oneLine)})`);
  console.log(`  with an untranslated name   ${withUntranslated} (${pct(withUntranslated)})`);
  console.log(`  untranslated names          ${names.size} distinct, ${occurrences} times`);
  console.log(`  name occurrences, no term   ${nameTokens ? Math.round((100 * occurrences) / nameTokens) : 0}% of ${nameTokens}`);
  console.log(`  longest line                ${longest.length} characters (${longest.where})`);
  if (showNames) {
    for (const [n, c] of [...names].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))) console.log(`    ${String(c).padStart(5)}  ${n}`);
  }
}
