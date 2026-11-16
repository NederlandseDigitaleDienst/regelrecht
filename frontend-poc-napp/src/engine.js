/**
 * In-browser regelrecht engine (WASM) with the NAPP law corpus pre-loaded.
 *
 * The WASM package is built via `just wasm` into public/wasm/pkg/. The law
 * YAML files are bundled as raw strings at build time, so the scenario runner
 * works without backend.
 */
import { b } from './basePad.js';

import wppYaml from '../../corpus-poc/napp/law/wet_op_de_politieke_partijen/2026-01-01.yaml?raw';
import regelingYaml from '../../corpus-poc/napp/law/regeling_subsidiebedragen/2026-01-01.yaml?raw';
import besluitDecentraalYaml from '../../corpus-poc/napp/law/besluit_subsidiering_decentrale_politieke_partijen/2026-01-01.yaml?raw';
import awbYaml from '../../corpus-poc/napp/law/algemene_wet_bestuursrecht/1994-01-01.yaml?raw';
import termijnenwetYaml from '../../corpus-poc/napp/law/algemene_termijnenwet/1964-04-01.yaml?raw';
import kieswetYaml from '../../corpus-poc/napp/law/kieswet/1989-09-28.yaml?raw';

// De wetten die napp uitvoert, in laadvolgorde. De scenariotests laden
// dezelfde lijst in hun engine, zodat test en browser niet uit elkaar lopen.
const wetten = [wppYaml, regelingYaml, besluitDecentraalYaml, awbYaml, termijnenwetYaml, kieswetYaml];

/** Laad de napp-wetten in een verse engine en geef die terug. */
export function laadWetten(engine) {
  for (const yaml of wetten) engine.loadLaw(yaml);
  return engine;
}

let enginePromise = null;

export function getEngine() {
  if (enginePromise) return enginePromise;
  enginePromise = (async () => {
    const jsRes = await fetch(b('/wasm/pkg/regelrecht_engine.js'));
    if (!jsRes.ok) {
      throw new Error(
        'WASM-engine niet gevonden. Bouw hem eerst met `just wasm`.',
      );
    }
    const jsText = await jsRes.text();
    const blob = new Blob([jsText], { type: 'application/javascript' });
    const blobUrl = URL.createObjectURL(blob);
    const wasm = await import(/* @vite-ignore */ blobUrl);
    URL.revokeObjectURL(blobUrl);
    await wasm.default(b('/wasm/pkg/regelrecht_engine_bg.wasm'));
    return laadWetten(new wasm.WasmEngine());
  })();
  return enginePromise;
}
