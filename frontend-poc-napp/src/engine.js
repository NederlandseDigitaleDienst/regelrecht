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

// The laws napp executes, in load order. The scenario tests load the same
// list into their engine, so the tests and the browser cannot drift apart.
const laws = [wppYaml, regelingYaml, besluitDecentraalYaml, awbYaml, termijnenwetYaml, kieswetYaml];

/** Load the napp laws into a fresh engine and return it. */
export function loadLaws(engine) {
  for (const yaml of laws) engine.loadLaw(yaml);
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
    return loadLaws(new wasm.WasmEngine());
  })();
  return enginePromise;
}
