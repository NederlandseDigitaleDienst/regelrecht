/**
 * Welke Nederlandse tekst nog rechtstreeks in een scherm staat.
 *
 * `check-i18n.mjs` bewaakt wat er in de woordenboeken staat; dit zoekt het
 * omgekeerde: een zin die nooit een sleutel heeft gekregen en dus in beide
 * talen Nederlands blijft. Dat is geen fout die iets breekt, en daarom is dit
 * een rapport en geen poort — maar het is precies wat je wilt zien voordat je
 * zegt dat een scherm vertaald is.
 *
 * De heuristiek is grof met opzet: liever een paar valse treffers die een mens
 * wegstreept dan een gemiste zin die niemand meer opmerkt.
 *
 * Read-only.
 *
 * Usage:
 *   node scripts/i18n-untranslated.mjs            # per bestand, met regelnummer
 *   node scripts/i18n-untranslated.mjs --count    # alleen de telling per bestand
 */
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const srcDir = resolve(here, '..', 'src');
const i18nDir = join(srcDir, 'i18n');

function walk(dir, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (/\.(vue|js|ts)$/.test(full) && !/\.test\.(js|ts)$/.test(full)) out.push(full);
  }
  return out;
}

// Woorden die een zin Nederlands maken en die in het Engels niet bestaan of
// iets anders betekenen. Een zin met een van deze erin is vrijwel zeker
// onvertaald; een zin zonder kan nog steeds Nederlands zijn, dus dit vindt niet
// alles. Het vindt wel genoeg om een scherm mee af te lopen.
const DUTCH = /\b(de|het|een|en|van|voor|niet|geen|met|aan|op|bij|door|naar|uit|wordt|worden|kan|kunnen|moet|moeten|heeft|hebben|is|zijn|deze|dit|die|dat|je|jij|jouw|wat|welke|waar|hoe|nog|al|ook|maar|dus|want|omdat|zodat)\b/i;

const attr = /(?:^|\s)(?:text|label|supporting-text|accessible-label|overline|details|placeholder|dismiss-text|title|subtitle)="([^"{}]{4,})"/g;
const jsString = /'([A-Z][a-zé][^']{6,})'/g;

const rows = [];
for (const file of walk(srcDir)) {
  if (file.startsWith(i18nDir)) continue;
  const rel = relative(srcDir, file);
  const lines = readFileSync(file, 'utf8').split('\n');
  for (const [i, line] of lines.entries()) {
    const trimmed = line.trim();
    // Commentaar is voor ontwikkelaars en blijft Nederlands.
    if (trimmed.startsWith('//') || trimmed.startsWith('*') || trimmed.startsWith('/*') || trimmed.startsWith('<!--')) continue;
    for (const re of [attr, jsString]) {
      re.lastIndex = 0;
      let m;
      while ((m = re.exec(line))) {
        const text = m[1];
        if (!DUTCH.test(text)) continue;
        // Een icoonnaam of een enum is geen zin.
        if (/^[a-z0-9-]+$/.test(text)) continue;
        rows.push({ file: rel, line: i + 1, text });
      }
    }
  }
}

if (process.argv.includes('--count')) {
  const per = new Map();
  for (const r of rows) per.set(r.file, (per.get(r.file) ?? 0) + 1);
  for (const [file, n] of [...per.entries()].sort((a, b) => b[1] - a[1])) {
    console.log(`${String(n).padStart(4)}  ${file}`);
  }
} else {
  for (const r of rows) console.log(`${r.file}:${r.line}  ${r.text}`);
}
console.error(`\n${rows.length} mogelijk onvertaalde teksten in ${new Set(rows.map((r) => r.file)).size} bestand(en).`);
