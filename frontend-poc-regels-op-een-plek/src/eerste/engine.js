// De RegelRecht-engine in de browser, met hetzelfde laadpatroon als de andere pocs
// (frontend-poc-terugbetaalregimes/src/engine/useEngine.js).
//
// Vite staat geen import uit /public toe, dus de JS-glue gaat via een blob-URL. De WASM
// wordt één keer geladen. Elke regelset krijgt een eigen engine: de kolommen tijdelijk en
// nieuw rekenen los van elkaar, zonder voorrang. De engine en versie.json komen uit deze
// repository (scripts/copy-assets.js), de regels uit de bron (bron.js).
import { b } from './basePad.js';
import { bestand, json } from '../bron.js';

let wasmModule = null;

async function tekst(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Ophalen mislukt (${res.status}): ${url}`);
  return res.text();
}

async function laadWasm() {
  if (wasmModule) return wasmModule;
  const blob = new Blob([await tekst(b('wasm/pkg/regelrecht_engine.js'))], { type: 'application/javascript' });
  const url = URL.createObjectURL(blob);
  const wasm = await import(/* @vite-ignore */ url);
  URL.revokeObjectURL(url);
  await wasm.default({ module_or_path: b('wasm/pkg/regelrecht_engine_bg.wasm') });
  wasmModule = wasm;
  return wasm;
}

/** Index van alle regelingen in de bron. */
export async function regelIndex() {
  return json('laws/index.json');
}

/** Versie van de engine waarmee gerekend wordt (versie.json, door copy-assets.js). */
export async function engineVersie() {
  return JSON.parse(await tekst(b('versie.json')));
}

/** Nieuwe engine met de gegeven regelingen geladen. `paden` komen uit regelIndex(). */
export async function maakEngine(paden = []) {
  const wasm = await laadWasm();
  const engine = new wasm.WasmEngine();
  for (const pad of paden) engine.loadLaw(bestand(pad));
  return engine;
}
