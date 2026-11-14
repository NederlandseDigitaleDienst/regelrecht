/**
 * Zet de gegevens uit corpus-poc/wetsgeschiedenis/gegevens/ in public/gegevens/,
 * waar de pagina ze ophaalt.
 *
 * dossiers.json noemt elk dossier en elke wet op de pagina; elk daarvan heeft
 * een eigen <id>.json. Ontbreekt er een, dan stopt de build: stil overslaan
 * haalt een keuze uit de lijst weg, en dat merkt niemand tot de demo.
 */
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const bron = resolve(appRoot, '..', 'corpus-poc', 'wetsgeschiedenis', 'gegevens');
const doel = resolve(appRoot, 'public', 'gegevens');

const index = resolve(bron, 'dossiers.json');
if (!existsSync(index)) {
  console.error(`✗ ${index} ontbreekt`);
  process.exit(1);
}
const dossiers = JSON.parse(readFileSync(index, 'utf8'));
const ontbreekt = dossiers.map((d) => `${d.dossier}.json`).filter((f) => !existsSync(resolve(bron, f)));
if (ontbreekt.length) {
  console.error(`✗ dossiers.json noemt bestanden die er niet zijn: ${ontbreekt.join(', ')}`);
  process.exit(1);
}
rmSync(doel, { recursive: true, force: true });
mkdirSync(doel, { recursive: true });
for (const f of ['dossiers.json', ...dossiers.map((d) => `${d.dossier}.json`)]) {
  cpSync(resolve(bron, f), resolve(doel, f));
}
console.log(`✓ ${dossiers.length} dossiers naar public/gegevens/`);
