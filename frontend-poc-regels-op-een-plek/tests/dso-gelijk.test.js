// Het DSO-spoor rekent in de browser opnieuw (Simuleren, Trace). Deze test rekent elk punt met de
// engine van deze repository en vergelijkt met de vooraf gerekende uitkomst in de bron: met de
// standaardinstelling moet er hetzelfde uitkomen. De bron staat niet in deze repository; zonder
// POC_BRON_SPOORZONE (een lokale map met de bron) slaat de test zich over.
import { existsSync, readFileSync } from 'fs';
import { resolve } from 'path';
import { pathToFileURL } from 'url';
import { expect, it } from 'vitest';
import { bestandenUitMap } from '../scripts/bron-dev.js';
import { bestand, json, zetBron } from '../src/bron.js';
import { AANNAMES_STANDAARD, BOUWPLAN_STANDAARD, maakMotor, rekenVak } from '../src/sim.js';

const BRON = process.env.POC_BRON_SPOORZONE;
const PKG = resolve(import.meta.dirname, '..', 'public', 'wasm', 'pkg');

it.skipIf(!BRON)('elk punt geeft met de engine van deze repository de vooraf gerekende uitkomst', async () => {
  if (!existsSync(resolve(PKG, 'regelrecht_engine_bg.wasm'))) throw new Error(`geen engine in ${PKG}: draai eerst node scripts/copy-assets.js`);
  zetBron({ bestanden: bestandenUitMap(BRON) });
  const wasm = await import(pathToFileURL(resolve(PKG, 'regelrecht_engine.js')).href);
  await wasm.default({ module_or_path: readFileSync(resolve(PKG, 'regelrecht_engine_bg.wasm')) });
  const sim = json('sim.json');
  const demo = json('demo.json');
  const m = maakMotor(wasm, sim, json('regels/lijst.json').map((n) => bestand(`regels/${n}`)));
  const verschil = [];
  let gerekend = 0;
  demo.cellen.forEach((c, i) => {
    const r = rekenVak(m, i, c.rd, { ...BOUWPLAN_STANDAARD }, { ...AANNAMES_STANDAARD });
    if (r.geldend) gerekend++;
    for (const k of ['geldend', 'beoogd', 'totaal']) if (r[k] !== c.dso[k]) verschil.push(`${i} ${k}: ${r[k]} ≠ ${c.dso[k]}`);
  });
  expect(demo.cellen.length).toBeGreaterThan(1000);
  expect(gerekend).toBeGreaterThan(1000);
  expect(verschil).toEqual([]);
}, 300000);
