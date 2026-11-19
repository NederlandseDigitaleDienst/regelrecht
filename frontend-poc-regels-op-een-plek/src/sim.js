// Simulatie: het DSO-spoor opnieuw rekenen in de browser, met een ander bouwplan of andere aannames.
// Zelfde toets (dso/toets.js), zelfde YAML (regels/ in de bron) en dezelfde RegelRecht-engine (WASM)
// als de vooraf gerekende uitkomst; alleen de invoer verschilt. De plannen en artikelen per vak
// komen uit sim.json in de bron; tests/dso-gelijk.test.js bewaakt dat die dezelfde uitkomst geven.
import { reactive } from 'vue';
import { gunstig, rekenCel } from './dso/toets.js';
import { bestand, json } from './bron.js';
import { b } from '@eerste/basePad.js';
// Eén bouwplan voor beide sporen: hetzelfde object als in Toets en Simuleren van de eerste PoC.
// Wie daar het aantal lagen verandert, ziet het DSO-spoor met hetzelfde bouwplan rekenen.
import { bouwplan } from '@eerste/toets/bouwplan.js';

export { bouwplan };
// De peildatum van de regels in het DSO-spoor: de dag waarop ze zijn opgehaald.
export const DATUM = '2026-11-16';
export const BOUWPLAN_STANDAARD = { lagen: 5, meterPerLaag: 3.2, woningtype: 'gestapeld', aantalWoningen: 20 };
export const AANNAMES_STANDAARD = { gegeven: 'gunstig', welstand: 'gunstig', bodem: 'gunstig', funderingsdiepte_m: 18 };
export const aannames = reactive({ ...AANNAMES_STANDAARD });

export const WONINGTYPES = [
  { waarde: 'gestapeld', tekst: 'Gestapeld' },
  { waarde: 'aaneengebouwd', tekst: 'Aaneen' },
  { waarde: 'twee-aaneen', tekst: 'Twee-aaneen' },
  { waarde: 'vrijstaand', tekst: 'Vrijstaand' },
];
export const KEUZE = [
  { waarde: 'gunstig', tekst: 'Gunstig' },
  { waarde: 'onbekend', tekst: 'Onbekend' },
  { waarde: 'ongunstig', tekst: 'Ongunstig' },
];
const WAARDE = { gunstig: true, onbekend: null, ongunstig: false };

export function standaard() {
  Object.assign(bouwplan, BOUWPLAN_STANDAARD);
  Object.assign(aannames, AANNAMES_STANDAARD);
}
export const isStandaard = () => Object.entries(BOUWPLAN_STANDAARD).every(([k, v]) => bouwplan[k] === v)
  && Object.entries(AANNAMES_STANDAARD).every(([k, v]) => aannames[k] === v);
export const hoogte = (bp = bouwplan) => Math.round(bp.lagen * bp.meterPerLaag * 100) / 100;

/** De aannames zoals rekenCel ze leest; onbekend is null en telt als onthouding. */
export function naarAannames(bp = bouwplan, a = aannames) {
  return {
    ...gunstig(bp),
    gegeven: WAARDE[a.gegeven], welstand: WAARDE[a.welstand], bodem: WAARDE[a.bodem],
    funderingsdiepte_m: Number(a.funderingsdiepte_m), artikel: {},
  };
}

// --- Engine en invoer, één keer geladen.
let motor = null;
async function tekst(url) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`Ophalen mislukt (${r.status}): ${url}`);
  return r.text();
}
// De plannen en artikelen per vak en de index van artikel naar YAML (sim.json, uit de bron).
let simBelofte = null;
export const laadSim = () => (simBelofte ??= Promise.resolve().then(() => json('sim.json')));

export function laadMotor() {
  motor ??= (async () => {
    const [sim, lijst, glue] = await Promise.all([
      laadSim(),
      json('regels/lijst.json'),
      tekst(b('wasm/pkg/regelrecht_engine.js')),
    ]);
    // Vite staat geen import uit /public toe; de JS-glue gaat via een blob-URL, zoals in de eerste PoC.
    const url = URL.createObjectURL(new Blob([glue], { type: 'application/javascript' }));
    const wasm = await import(/* @vite-ignore */ url);
    URL.revokeObjectURL(url);
    await wasm.default({ module_or_path: b('wasm/pkg/regelrecht_engine_bg.wasm') });
    return maakMotor(wasm, sim, lijst.map((n) => bestand(`regels/${n}`)));
  })();
  return motor;
}

/**
 * De motor van de simulatie: per regeling een engine, met voer (de uitkomst: true, false of
 * null voor onbekend) en trace (dezelfde aanroep met de trace van de engine). `wasm` is de
 * geladen WASM-module, `regels` de YAML-teksten. Los van het laden, zodat een test hem ook buiten
 * de browser kan bouwen.
 */
export function maakMotor(wasm, sim, regels) {
  const engines = new Map();
  for (const t of regels) {
    const e = new wasm.WasmEngine();
    e.loadLaw(t);
    engines.set(t.match(/^\$id: (\S+)/m)[1], e);
  }
  const cache = new Map();
  const voer = (law, output, params) => {
    const k = `${law}|${output}|${JSON.stringify(params)}`;
    if (cache.has(k)) return cache.get(k);
    let w = null;
    try {
      const v = engines.get(law).execute(law, output, params, DATUM).outputs?.[output];
      w = v === true || v === false ? v : null;
    } catch { w = null; }
    cache.set(k, w);
    return w;
  };
  const trace = (law, output, params) => {
    try {
      return engines.get(law).executeWithTrace(law, output, params, DATUM).trace_text ?? '(geen trace)';
    } catch (e) {
      return e?.trace_text ?? `Fout: ${e?.message ?? e}`;
    }
  };
  const aantalGegeven = Object.values(sim.index).filter((x) => x.vorm === 'GEGEVEN').length;
  return { sim, voer, trace, regelingen: regels.length, aantalGegeven, aantalArtikelen: Object.keys(sim.index).length };
}

/** Reken vak i (index in demo.json en sim.json) met het gegeven bouwplan en de aannames. */
export function rekenVak(m, i, rd, bp = bouwplan, a = aannames) {
  const c = m.sim.cellen[i];
  const sleutels = [...m.sim.gemeen, ...c.k].map((n) => m.sim.sleutels[n]);
  const plan = { ...bp };
  return rekenCel({ cel: { rd, rp: c.rp }, sleutels, artikelen: m.sim.artikelen, index: m.sim.index, voer: m.voer, bouwplan: plan, aannames: naarAannames(plan, a) });
}

/**
 * De trace van het DSO-spoor op vak i: dezelfde berekening als rekenVak, met de standaardinstelling
 * van de vooraf gerekende uitkomst (5 lagen van 3,2 m, open gegevens gunstig). Per aanroep van de
 * engine: het artikel (sleutel), de uitvoer, de invoer, de uitkomst en de trace, in volgorde.
 */
export function traceVak(m, i, rd) {
  const c = m.sim.cellen[i];
  const sleutels = [...m.sim.gemeen, ...c.k].map((n) => m.sim.sleutels[n]);
  const naarSleutel = new Map(Object.entries(m.sim.index).map(([k, ix]) => [`${ix.law}|${ix.output}`, k]));
  const aanroepen = new Map();
  const voer = (law, output, params) => {
    const waarde = m.voer(law, output, params);
    const k = `${law}|${output}|${JSON.stringify(params)}`;
    if (!aanroepen.has(k)) aanroepen.set(k, { sleutel: naarSleutel.get(`${law}|${output}`), law, output, params, waarde });
    return waarde;
  };
  const plan = { ...BOUWPLAN_STANDAARD };
  rekenCel({ cel: { rd, rp: c.rp }, sleutels, artikelen: m.sim.artikelen, index: m.sim.index, voer, bouwplan: plan, aannames: naarAannames(plan, { ...AANNAMES_STANDAARD }) });
  return [...aanroepen.values()].map((a) => ({ ...a, trace: m.trace(a.law, a.output, a.params) }));
}

// --- De YAML achter een artikel: het stuk van het regelbestand met dat artikel, letterlijk.
const yamlBestanden = new Map();
/**
 * Het artikel uit de YAML van het DSO-spoor: { bestand, yaml } of { reden } als er geen YAML is.
 * Een artikel begint met "- number:" onder articles; het juiste heeft zijn uitvoer als endpoint.
 */
export async function yamlVanArtikel(sleutel) {
  const ix = (await laadSim()).index[sleutel];
  if (!ix) return { reden: 'Dit artikel rekent het DSO-spoor niet: het heeft geen YAML.' };
  if (ix.vorm === 'geen-tekst' || !ix.law) return { reden: 'Geen YAML: het artikel heeft geen tekst in het DSO (bijvoorbeeld gereserveerd).' };
  if (!yamlBestanden.has(ix.bestand)) yamlBestanden.set(ix.bestand, bestand(`regels/${ix.bestand}`));
  const regels = yamlBestanden.get(ix.bestand).split('\n');
  const starts = regels.flatMap((r, n) => (r.startsWith('- number:') ? [n] : []));
  for (let j = 0; j < starts.length; j++) {
    const blok = regels.slice(starts[j], starts[j + 1] ?? regels.length);
    if (blok.some((r) => r.trim() === `endpoint: ${ix.output}`)) return { bestand: ix.bestand, yaml: blok.join('\n').trimEnd() };
  }
  return { reden: `Artikel niet gevonden in ${ix.bestand}.` };
}

/** Alle vakken van het raster, in stukjes zodat de pagina blijft reageren. */
export async function rekenKaart(m, cellen, { opVoortgang = () => {}, gestopt = () => false } = {}) {
  const bp = { ...bouwplan }; const a = { ...aannames };
  const uit = new Array(cellen.length).fill(null);
  for (let i = 0; i < cellen.length; i++) {
    if (gestopt()) return null;
    if (!cellen[i].extra) {
      const r = rekenVak(m, i, cellen[i].rd, bp, a);
      uit[i] = { geldend: r.geldend, beoogd: r.beoogd, totaal: r.totaal };
    }
    if (i % 60 === 0) { opVoortgang(i / cellen.length); await new Promise((ok) => setTimeout(ok, 0)); }
  }
  return uit;
}

/** De toets op één vak voor 1 tot en met 15 lagen: bij hoeveel lagen slaat een regel om? */
export function lagenScan(m, i, rd) {
  const lagen = Array.from({ length: 15 }, (_, n) => n + 1);
  const runs = lagen.map((n) => rekenVak(m, i, rd, { ...bouwplan, lagen: n }));
  return { lagen, hoogtes: lagen.map((n) => hoogte({ ...bouwplan, lagen: n })), runs };
}
