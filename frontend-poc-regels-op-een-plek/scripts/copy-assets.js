/**
 * Zet in public/ wat de app uit deze repository haalt:
 *
 *  - de WASM-engine uit `just wasm-build`             -> public/wasm/pkg/
 *  - versie.json: engineversie, hoogste schema, commit -> public/versie.json
 *  - favicon en touch icon uit het design system       -> public/
 *
 * De regels, de kaart en de uitkomsten staan niet in deze repository: die komen tijdens het
 * draaien uit de bron (src/bron.js). Ontbreekt de engine, dan stopt de build: stil overslaan
 * valt pas op in de demo.
 */
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'fs';
import { execFileSync } from 'child_process';
import { dirname, resolve } from 'path';
import { fileURLToPath } from 'url';

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(appRoot, '..');
const pub = resolve(appRoot, 'public');
const stop = (melding) => {
  console.error(melding);
  process.exit(1);
};

// --- wasm ---
const wasmSrc = resolve(repoRoot, 'frontend', 'public', 'wasm', 'pkg');
if (!existsSync(resolve(wasmSrc, 'regelrecht_engine_bg.wasm'))) {
  stop(`wasm pkg niet gevonden in ${wasmSrc}: draai eerst 'just wasm-build'.`);
}
mkdirSync(resolve(pub, 'wasm', 'pkg'), { recursive: true });
cpSync(wasmSrc, resolve(pub, 'wasm', 'pkg'), { recursive: true });

// --- versie ---
// Wat er te weten is over de engine en het schema. In de Docker-build van het portaal staan
// packages/ en schema/ niet in de stap die de frontends bouwt, en is er geen git: dan blijft een
// veld leeg en toont de voetregel het niet. De engineversie geeft de engine zelf ook.
const cargoPad = resolve(repoRoot, 'packages', 'engine', 'Cargo.toml');
const engineVersie = existsSync(cargoPad) ? (readFileSync(cargoPad, 'utf8').match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? '') : '';
const schemaDir = resolve(repoRoot, 'schema');
const schemas = existsSync(schemaDir)
  ? readdirSync(schemaDir).filter((n) => /^v\d+\.\d+\.\d+$/.test(n))
    .sort((a, b) => a.slice(1).localeCompare(b.slice(1), 'en', { numeric: true }))
  : [];
let commit = process.env.GIT_SHA ?? '';
if (!commit) {
  try {
    commit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repoRoot, encoding: 'utf8' }).trim();
  } catch {
    commit = ''; // in de Docker-build is er geen git
  }
}
writeFileSync(resolve(pub, 'versie.json'), JSON.stringify({
  engine_versie: engineVersie,
  schema_hoogste: schemas.at(-1) ?? '',
  commit,
}, null, 2));

// --- favicon en touch icon uit het design system (rijkswapen op lintblauw) ---
// Uit het pakket, niet als kopie in git: dan blijft NOTICES.md van het pakket de enige plek
// voor de gebruiksvoorwaarden. In de npm-workspace staat het pakket in de node_modules van de
// repository; met een eigen versie ervan in die van de app.
const nldd = [appRoot, repoRoot]
  .map((r) => resolve(r, 'node_modules', '@nldd', 'design-system', 'dist'))
  .find((d) => existsSync(resolve(d, 'favicon.svg')));
if (!nldd) stop("favicon.svg niet gevonden in @nldd/design-system: draai eerst 'npm install'.");
for (const f of ['favicon.svg', 'touch-icon.png']) cpSync(resolve(nldd, f), resolve(pub, f));

console.log(`wasm: engine ${engineVersie || '?'}, schema t/m ${schemas.at(-1) ?? '?'}${commit ? `, commit ${commit.slice(0, 9)}` : ''}`);
