// Kaartlaag Uitkomst: de toets op een raster over het zoekgebied, met het bouwplan en de
// aannames uit Simuleren. Per cel de uitkomst in het midden van de
// cel. Deze module rekent zelf niets: elke cel gaat door rekenToets, net als een klik.
//
// Twee weergaven:
//   geldend  het recht dat nu geldt: het tijdelijk deel, of het nieuwe deel waar een
//            vastgesteld TAM-plan ligt (dan is het tijdelijk deel niet van toepassing)
//   beoogd   alleen waar een ontwerp-TAM ligt: de kolom nieuw met dat ontwerp
import { naarRd, puntInVlak, vanRd } from './geo.js';
import { bepaalStapel } from './stapel.js';
import { AFGELEID, veldVoor } from '../toets/aannames.js';
import { rekenToets } from '../toets/toets.js';

/** Klassen op de kaart. Statuskleuren, altijd met tekst in de legenda. */
export const UITKOMST_KLASSEN = [
  { klasse: 'toegestaan', tekst: 'Toegestaan', kleur: '#0ca30c' },
  { klasse: 'onthouding', tekst: 'Onthouding: er ontbreekt een gegeven', kleur: '#fab219' },
  { klasse: 'bron', tekst: 'Onthouding: regels alleen als scan (Het Ambacht 2009), niet in te vullen', kleur: '#8c6d1f' },
  { klasse: 'niet', tekst: 'Niet toegestaan', kleur: '#d03b3b' },
  { klasse: 'open', tekst: 'Toets rekent hier niet: plan niet in YAML, of geen regels', kleur: '#8a8a86' },
];

const NAAR_KLASSE = { toegestaan: 'toegestaan', 'niet toegestaan': 'niet', onthouding: 'onthouding' };

/** De cellen van het raster met hun stapel. Hangt niet af van het bouwplan; één keer maken. */
export function rasterCellen({ index, lagen, zoekgebied }, cel = 40) {
  const vlak = zoekgebied.features[0].geometry;
  const rd = vlak.coordinates[0].map((ll) => naarRd(ll));
  const xs = rd.map((c) => c[0]);
  const ys = rd.map((c) => c[1]);
  const cellen = [];
  for (let x = Math.floor(Math.min(...xs) / cel) * cel; x < Math.max(...xs); x += cel) {
    for (let y = Math.floor(Math.min(...ys) / cel) * cel; y < Math.max(...ys); y += cel) {
      const midden = vanRd([x + cel / 2, y + cel / 2]);
      if (!puntInVlak(midden, vlak)) continue;
      const hoek = [[x, y], [x + cel, y], [x + cel, y + cel], [x, y + cel], [x, y]].map(vanRd);
      cellen.push({ midden, hoek, stapel: bepaalStapel(midden, index, lagen) });
    }
  }
  return cellen;
}

/**
 * Verfijn het raster waar de regels binnen een vak verschillen. Een vak van 40 meter krijgt de
 * uitkomst van zijn midden; een smal bouwvlak of een kleine bestemming valt daar tussendoor
 * (in 't Goeie Spoor liet de kaart zonder verfijning nergens "toegestaan" zien, een klik wel).
 * Daarom per vak de stapel op de middens van de deelvakken. Is die overal gelijk, dan blijft het
 * vak heel. Anders komen de deelvakken ervoor in de plaats, elk met gewicht 1/n in de telling.
 * Het zoekgebied blijft bepaald door het midden van het grote vak, zoals in rasterCellen.
 */
// De stapel zonder de plek zelf: twee plekken met dezelfde plannen en dezelfde soort vlakken, artikelen
// en maten zijn voor de toets gelijk. Wat alleen het ene kaartobject van het andere onderscheidt, valt weg.
const ZONDER = new Set(['punt', 'id', 'ha', 'verwijzing']);
const teken = (stapel) => JSON.stringify(stapel, (k, v) => (ZONDER.has(k) ? undefined : k === 'plan' ? v?.id : v));

export function verfijnCellen({ index, lagen }, cellen, delen = 4) {
  const uit = [];
  for (const c of cellen) {
    const [a, , b] = c.hoek.map(naarRd); // linksonder en rechtsboven in RD
    const stap = (b[0] - a[0]) / delen;
    const sub = [];
    for (let i = 0; i < delen; i++) for (let j = 0; j < delen; j++) {
      const x = a[0] + i * stap; const y = a[1] + j * stap;
      const midden = vanRd([x + stap / 2, y + stap / 2]);
      const hoek = [[x, y], [x + stap, y], [x + stap, y + stap], [x, y + stap], [x, y]].map(vanRd);
      const stapel = bepaalStapel(midden, index, lagen);
      sub.push({ midden, hoek, stapel, teken: teken(stapel), gewicht: 1 / (delen * delen) });
    }
    // Overal gelijk: het vak blijft heel, met de stapel van een deelvak (het midden van het vak ligt op een grens ertussen).
    if (sub.every((d) => d.teken === sub[0].teken)) uit.push({ ...c, stapel: sub[0].stapel, teken: sub[0].teken });
    else uit.push(...sub);
  }
  return uit;
}

/** Klasse van één cel uit de uitkomst van rekenToets. null: niet tekenen. */
export function klasseVan(stapel, toets, weergave = 'geldend') {
  const eind = (k) => NAAR_KLASSE[k.eindoordeel?.uitkomst] ?? 'open';
  if (weergave === 'beoogd') {
    if (!stapel.beoogd || stapel.tam) return null;
    return toets.nieuw.status === 'gerekend' ? eind(toets.nieuw) : 'open';
  }
  const k = stapel.tam ? toets.nieuw : toets.tijdelijk;
  if (k.status === 'gerekend') return eind(k);
  if (k.status === 'bron-ontbreekt') return 'bron';
  return 'open';
}

/**
 * Waarom een cel onthouding is: de velden uit aannames.js die nog leeg zijn, een maat die niet
 * op de kaart staat, of een regel die nog niet in YAML staat. Per cel elke reden één keer.
 */
export function redenenVan(stapel, toets, weergave = 'geldend') {
  const k = weergave === 'beoogd' || stapel.tam ? toets.nieuw : toets.tijdelijk;
  const uit = new Set();
  for (const naam of (k.stappen ?? []).flatMap((s) => s.ontbrekend ?? [])) {
    if (AFGELEID.has(naam)) continue;
    uit.add(veldVoor(naam)?.id ?? 'kaartfeit');
  }
  if (k.nietGemodelleerd?.length) uit.add('niet-in-yaml');
  return uit;
}

/**
 * Reken het raster door. Geeft tussendoor de beurt terug aan de browser, zodat de kaart
 * bruikbaar blijft, en stopt als `gestopt()` waar wordt (een nieuwere berekening).
 * @returns {Promise<{features: object[], telling: object, ontbreekt: object}|null>}  null als gestopt
 */
export async function rekenUitkomst({ cellen, bouwplan, aannames, weergave = 'geldend', datum, index, varianten, maakEngine, opVoortgang = () => {}, gestopt = () => false }) {
  const features = [];
  const telling = Object.fromEntries(UITKOMST_KLASSEN.map((k) => [k.klasse, 0]));
  const bekend = new Map(); // teken van de stapel naar klasse en redenen, binnen deze berekening
  const ontbreekt = {}; // veld-id (of 'kaartfeit', 'niet-in-yaml') naar aantal cellen
  for (let i = 0; i < cellen.length; i++) {
    if (i % 40 === 0) {
      opVoortgang(i / cellen.length);
      await new Promise((r) => setTimeout(r, 0));
      if (gestopt()) return null;
    }
    const { stapel, hoek, gewicht = 1, teken: sleutel } = cellen[i];
    if (weergave === 'beoogd' && (!stapel.beoogd || stapel.tam)) continue;
    // Gelijke stapel, gelijke uitkomst: één keer rekenen. Niet bij een TAM-plan of een ontwerp, want
    // daar tellen ook afstanden vanaf de plek zelf mee (TAM Het Ambacht 70 en 71).
    const deelbaar = sleutel && !stapel.tam && !stapel.beoogd;
    let r = deelbaar ? bekend.get(sleutel) : undefined;
    if (!r) {
      const toets = await rekenToets({ stapel, bouwplan, aannames, datum, index, varianten, maakEngine });
      const klasse = klasseVan(stapel, toets, weergave);
      r = { klasse, redenen: klasse === 'onthouding' ? [...redenenVan(stapel, toets, weergave)] : [] };
      if (deelbaar) bekend.set(sleutel, r);
    }
    const { klasse } = r;
    if (!klasse) continue;
    telling[klasse] += gewicht;
    for (const reden of r.redenen) ontbreekt[reden] = (ontbreekt[reden] ?? 0) + gewicht;
    features.push({ type: 'Feature', geometry: { type: 'Polygon', coordinates: [hoek] }, properties: { klasse } });
  }
  opVoortgang(1);
  // Deelvakken tellen als een deel van een vak; de telling blijft in hele vakken van het grove raster.
  for (const k of Object.keys(telling)) telling[k] = Math.round(telling[k]);
  for (const k of Object.keys(ontbreekt)) ontbreekt[k] = Math.max(1, Math.round(ontbreekt[k]));
  return { features, telling, ontbreekt };
}
