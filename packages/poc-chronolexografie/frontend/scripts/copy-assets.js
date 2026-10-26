/**
 * Zet in public/ wat de app in de browser draait:
 *
 *  - public/bundel.json: de bestanden die elke casus nodig heeft, per pad in
 *    de repository. Welke mappen dat zijn, staat bij `corpus` van
 *    chronolexografie in pocs/registry.yaml; een test in
 *    packages/poc-chronolexografie (src/bundel.rs) houdt die lijst gelijk met
 *    wat de app per casus uit de casussen en de wet afleidt. Elk bestand
 *    eronder gaat mee, behalve de scenario's.
 *  - public/bronnen.json: de namen van de bronnen bij `sources` van
 *    chronolexografie. De app haalt elke bron op bron/<naam> wanneer zij
 *    laadt (src/bronnen.js); het portaal serveert haar, niet de build.
 *  - public/wasm/pkg/: de cel en het spel als WASM, uit `just
 *    poc-chronolexografie-wasm` (of de wasm-builder in het poc-image).
 *
 * Een map uit het register die er niet is, of een ontbrekende WASM-build, is
 * een harde fout: stil overslaan levert een app op die pas in de browser
 * zegt dat er een cel of een wet ontbreekt.
 */
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import { dirname, relative, resolve } from 'path';
import { fileURLToPath } from 'url';
import * as yaml from 'js-yaml';

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
// De app staat in packages/poc-chronolexografie/frontend, naast de crate.
const repoRoot = resolve(appRoot, '..', '..', '..');

const register = yaml.load(readFileSync(resolve(repoRoot, 'pocs', 'registry.yaml'), 'utf8'));
const poc = register.pocs.find((p) => p.slug === 'chronolexografie');
if (!poc?.corpus?.length) {
  console.error('pocs/registry.yaml noemt bij chronolexografie geen corpus');
  process.exit(1);
}

/** Elk bestand onder `dir`, behalve wat onder een map `scenarios` staat. */
function bestanden(dir) {
  const uit = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const pad = resolve(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== 'scenarios') uit.push(...bestanden(pad));
    } else {
      uit.push(pad);
    }
  }
  return uit;
}

const bundel = {};
for (const map of poc.corpus) {
  const dir = resolve(repoRoot, map);
  if (!existsSync(dir) || !statSync(dir).isDirectory()) {
    console.error(`${map} uit pocs/registry.yaml is geen map in de repository`);
    process.exit(1);
  }
  for (const pad of bestanden(dir)) {
    bundel[relative(repoRoot, pad).split('\\').join('/')] = readFileSync(pad, 'utf8');
  }
}
mkdirSync(resolve(appRoot, 'public'), { recursive: true });
writeFileSync(resolve(appRoot, 'public', 'bundel.json'), JSON.stringify(bundel));
console.log(`bundel: ${Object.keys(bundel).length} bestand(en) uit ${poc.corpus.length} map(pen)`);
const bronnen = (poc.sources ?? []).map((s) => s.name);
writeFileSync(resolve(appRoot, 'public', 'bronnen.json'), JSON.stringify(bronnen));
console.log(`bronnen: ${bronnen.join(', ') || 'geen'}`);

const wasm = resolve(appRoot, 'public', 'wasm', 'pkg', 'regelrecht_poc_chronolexografie_bg.wasm');
if (!existsSync(wasm)) {
  console.error(`${relative(repoRoot, wasm)} ontbreekt: draai eerst 'just poc-chronolexografie-wasm'.`);
  process.exit(1);
}
console.log(`wasm: ${relative(repoRoot, wasm)}`);
