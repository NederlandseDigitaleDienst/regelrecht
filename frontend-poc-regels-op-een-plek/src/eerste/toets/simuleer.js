// Simuleren: de toets voor 1 tot en met 15 lagen, en per regel waar de uitkomst
// omslaat. Dit verzamelt alleen uitkomsten van de engine (via rekenToets); het rekent zelf niets.
import { OMGEVINGSPLAN } from './plannen.js';
import { rekenToets } from './toets.js';

/** Hergebruik engines met dezelfde regelingen; de engine houdt geen toestand tussen aanroepen. */
export function bewaarEngines(maakEngine) {
  const cache = new Map();
  return async (paden) => {
    const sleutel = paden.join('|');
    if (!cache.has(sleutel)) cache.set(sleutel, await maakEngine(paden));
    return cache.get(sleutel);
  };
}

function kolomRijen(runs, kolom) {
  const statussen = new Set(runs.map((r) => r[kolom].status));
  if (statussen.size === 1 && !statussen.has('gerekend')) {
    return { status: runs[0][kolom].status, toelichting: runs[0][kolom].toelichting, label: runs[0][kolom].label, rijen: [] };
  }
  const rijen = new Map();
  runs.forEach((run, i) => {
    for (const stap of run[kolom].stappen ?? []) {
      const uitvoer = stap.uitvoer && Object.keys(stap.uitvoer).length ? stap.uitvoer : { [stap.output]: stap };
      for (const [naam, l] of Object.entries(uitvoer)) {
        const sleutel = `${stap.regeling}|${stap.artikel}|${naam}`;
        if (!rijen.has(sleutel)) {
          rijen.set(sleutel, {
            regeling: stap.regeling, artikel: stap.artikel, output: naam,
            eindoordeel: stap.regeling === OMGEVINGSPLAN && naam === 'omgevingsvergunning_wordt_verleend',
            uitkomsten: Array(runs.length).fill('geen uitkomst'),
          });
        }
        rijen.get(sleutel).uitkomsten[i] = l.uitkomst;
      }
    }
  });
  const lijst = [...rijen.values()].sort((a, b) => Number(a.eindoordeel) - Number(b.eindoordeel));
  return { status: 'gerekend', label: runs[0][kolom].label, plan: runs[0][kolom].plan, rijen: lijst };
}

/** Plekken waar een rij van uitkomst wisselt, als { index, van, naar }. */
export function kantelpunten(rij) {
  const uit = [];
  for (let i = 1; i < rij.uitkomsten.length; i++) {
    if (rij.uitkomsten[i] !== rij.uitkomsten[i - 1]) uit.push({ index: i, van: rij.uitkomsten[i - 1], naar: rij.uitkomsten[i] });
  }
  return uit;
}

/**
 * @returns {{ lagen: number[], spelen: Set<string>, hoogtes: number[], tijdelijk: object, nieuw: object }}
 */
export async function simuleer({ stapel, bouwplan, aannames, van = 1, tot = 15, datum, index, varianten, maakEngine }) {
  const engine = bewaarEngines(maakEngine);
  const lagen = Array.from({ length: tot - van + 1 }, (_, i) => van + i);
  const runs = [];
  for (const n of lagen) {
    runs.push(await rekenToets({ stapel, bouwplan: { ...bouwplan, lagen: n }, aannames, datum, index, varianten, maakEngine: engine }));
  }
  // Welke gegevens op dit punt meetellen: wat ontbreekt, of wat als aanname is gelezen.
  const spelen = new Set(runs.flatMap((r) => ['tijdelijk', 'nieuw']
    .flatMap((k) => (r[k].stappen ?? []).flatMap((s) => [...(s.ontbrekend ?? []), ...(s.aangenomen ?? [])]))));
  return {
    lagen, spelen,
    hoogtes: runs.map((r) => r.bouwplan.bouwhoogte),
    tijdelijk: kolomRijen(runs, 'tijdelijk'),
    nieuw: kolomRijen(runs, 'nieuw'),
  };
}
