/**
 * Wat de woordenlijst van elke veldnaam maakt.
 *
 * The glossary translates words; the labels are composed from them, and a
 * composition can read badly while every word in it is right ("Has partner
 * income" is fine, "Meets the requirement of conduct" is not). This prints
 * every name with the label it produces, so that composing can be reviewed as
 * the thing a reader actually sees rather than as a word list.
 *
 * A name that composes is printed with `=`; one that falls back to Dutch with
 * `~`, which is how the demo shows it and what the next batch of glossary work
 * should pick up.
 *
 * Read-only.
 *
 * Usage:
 *   node scripts/i18n-labels.mjs             # every name, in the first translated locale
 *   node scripts/i18n-labels.mjs --locale=en # a specific locale
 *   node scripts/i18n-labels.mjs --fallback  # only the ones still in Dutch
 */
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import glossaries from '../src/i18n/glossary.generated.js';

const here = dirname(fileURLToPath(import.meta.url));
const lawsDir = resolve(here, '..', '..', 'corpus', 'demo', 'regulation', 'nl');

// Welke taal wordt nagelopen. De woordenlijst is per taal gegenereerd, dus dit
// script moet er een kiezen; zonder vlag is dat de eerste vertaalde taal, want
// zolang er maar één is scheelt dat een argument bij elke aanroep.
const localeArg = process.argv.find((a) => a.startsWith('--locale='))?.slice('--locale='.length);
const locale = localeArg ?? Object.keys(glossaries)[0];
const glossary = glossaries[locale];
if (!glossary) {
  console.error(`geen woordenlijst voor taal "${locale}"; beschikbaar: ${Object.keys(glossaries).join(', ') || '(geen)'}`);
  process.exit(1);
}

// Dezelfde afkortingenlijst als format.ts. Bewust gedupliceerd en niet
// geïmporteerd: format.ts hangt aan Vue en aan de i18n-module, en dit script
// draait waar geen frontend-afhankelijkheden staan.
const ABBREVIATIONS = new Map([
  ['agp', 'AGP'], ['aow', 'AOW'], ['bsn', 'BSN'], ['bbz', 'Bbz'], ['brp', 'BRP'],
  ['cbs', 'CBS'], ['haccp', 'HACCP'], ['kvk', 'KvK'], ['nvwa', 'NVWA'], ['sbi', 'SBI'],
  ['svh', 'SVH'], ['vog', 'VOG'], ['wia', 'WIA'], ['ww', 'WW'], ['zvw', 'Zvw'],
]);

function walk(dir, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (full.endsWith('.yaml')) out.push(full);
  }
  return out;
}

function capitalise(words) {
  const out = [...words];
  if (!out.length) return '';
  const first = out[0];
  out[0] = ABBREVIATIONS.has(String(first).toLowerCase()) ? first : first.charAt(0).toUpperCase() + first.slice(1);
  return out.join(' ');
}

function label(name) {
  const words = name.split('_');
  const dutch = capitalise(words.map((w) => ABBREVIATIONS.get(w.toLowerCase()) ?? w));
  const exact = glossary.names?.[name];
  if (exact) return { en: capitalise(String(exact).split(/\s+/)), composed: true };
  const parts = words.map((w) => glossary.words?.[w] ?? ABBREVIATIONS.get(w.toLowerCase()) ?? null);
  if (parts.every(Boolean)) return { en: capitalise(parts.join(' ').split(/\s+/)), composed: true };
  return { en: dutch, composed: false };
}

const names = new Set();
for (const file of walk(lawsDir)) {
  for (const line of readFileSync(file, 'utf8').split('\n')) {
    const m = /^\s*-\s+name:\s+([a-z][a-z0-9_]*)\s*$/.exec(line);
    if (m) names.add(m[1]);
  }
}

const onlyFallback = process.argv.includes('--fallback');
let composed = 0;
for (const name of [...names].sort()) {
  const r = label(name);
  if (r.composed) composed += 1;
  if (onlyFallback && r.composed) continue;
  console.log(`${r.composed ? '=' : '~'} ${name.padEnd(42)} ${r.en}`);
}
console.error(`\n${locale}: ${composed} van ${names.size} veldnamen stellen samen; ${names.size - composed} vallen terug op het Nederlands.`);
