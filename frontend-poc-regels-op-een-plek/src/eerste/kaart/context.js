// Contextlagen voor woningbouw op de kaart: wat de plankaart op elke plek zegt over bouwen,
// en waar de toets rekent. Dit is selectie uit de GML, geen toets: de maximale bouwhoogte is
// de maatvoering zoals die op de kaart staat, de bestemming de hoofdgroep uit IMRO. Of een
// bouwplan past, zegt alleen de engine (tab Toets).
//
// Welk plan op een plek geldt, volgt uit stapel.js. Een object uit de GML komt op de kaart als
// zijn plan geldt op het binnenpunt van dat object (eigenschap `punt`). Een groot vlak dat maar
// voor een deel is herzien, staat er dus helemaal op; het herziende plan ligt er dan bovenop,
// want de lagen worden van oud naar nieuw getekend. Een ontwerp-TAM geldt niet en staat
// alleen in de dekkingslaag (kolom nieuw, "beoogd").
import { naarRd, puntInVlak, vanRd } from './geo.js';
import { bepaalStapel } from './stapel.js';
import { dekkingOpPunt } from '../toets/toets.js';

/** Klassen voor de maximale bouwhoogte, in meters. Lagen bij 3,2 meter per laag. */
export const HOOGTE = [
  { tot: 9.6, tekst: 'tot 9,6 m (3 lagen)', kleur: '#86b6ef' },
  { tot: 12.8, tekst: '9,6 tot 12,8 m (4 lagen)', kleur: '#5598e7' },
  { tot: 16, tekst: '12,8 tot 16 m (5 lagen)', kleur: '#2a78d6' },
  { tot: 22.4, tekst: '16 tot 22,4 m (6 of 7 lagen)', kleur: '#1c5cab' },
  { tot: Infinity, tekst: 'meer dan 22,4 m (8 lagen of meer)', kleur: '#104281' },
];

/** Hoofdgroepen van bestemmingen in vier groepen. De hoofdgroep komt uit de GML. */
export const BESTEMMING = [
  { groep: 'wonen', tekst: 'Wonen', kleur: '#2a78d6', hoofdgroepen: ['wonen'] },
  { groep: 'gemengd', tekst: 'Gemengd of centrum', kleur: '#1baf7a', hoofdgroepen: ['gemengd', 'centrum'] },
  { groep: 'functie', tekst: 'Andere functie: maatschappelijk, bedrijf, horeca, sport en dergelijke', kleur: '#6250d6',
    hoofdgroepen: ['maatschappelijk', 'dienstverlening', 'horeca', 'kantoor', 'detailhandel', 'sport', 'bedrijf', 'bedrijventerrein', 'cultuur en ontspanning'] },
  { groep: 'open', tekst: 'Groen, water, verkeer', kleur: '#8a8a86', hoofdgroepen: ['groen', 'water', 'verkeer', 'tuin', 'natuur', 'agrarisch'] },
];

/** Dekking van de toets op een plek. Statuskleuren, altijd met tekst in de legenda. */
export const DEKKING = [
  { klasse: 'rekent', tekst: 'Toets rekent op het geldende recht', kleur: '#0ca30c' },
  { klasse: 'open', tekst: 'Plan of bestemming nog niet in YAML', kleur: '#8a8a86' },
  { klasse: 'bron', tekst: 'Bron ontbreekt of alleen een scan (het ontwerp-TAM rekent wel)', kleur: '#fab219' },
];

const NIET_GELDEND = new Set(['paraplu', 'facet', 'tam-ontwerp']);
const fc = (features) => ({ type: 'FeatureCollection', features });

export function hoogteKlasse(m) {
  return HOOGTE.findIndex((k) => m <= k.tot);
}

function maxBouwhoogte(props) {
  const w = (props.waarden ?? []).find((x) => x.type === 'maximum bouwhoogte (m)' && x.getal != null);
  return w?.getal;
}

/** Het plan dat op punt p geldt: een vastgesteld TAM-plan, anders het basisplan. */
function geldendPlan(p, index, lagen, cache) {
  const sleutel = p.join(',');
  if (!cache.has(sleutel)) {
    const s = bepaalStapel(p, index, lagen);
    cache.set(sleutel, (s.tam ?? s.basisplan)?.plan.id ?? null);
  }
  return cache.get(sleutel);
}

/**
 * @param {object} data  uit laden.js: { index, lagen, zoekgebied }

 */
export function bouwContext(data) {
  const { index, lagen, zoekgebied } = data;
  const cache = new Map();
  const hoogte = [];
  const bestemming = [];
  const bouwvlak = [];

  const plannen = index.plannen.filter((pl) => !NIET_GELDEND.has(pl.rol))
    .sort((a, b) => a.datum.localeCompare(b.datum));
  for (const plan of plannen) {
    for (const f of lagen.get(plan.id).features) {
      const pr = f.properties;
      if (!pr.punt || !['maatvoering', 'enkelbestemming', 'bouwvlak'].includes(pr.soort)) continue;
      let doel = null;
      let extra = {};
      if (pr.soort === 'maatvoering') {
        const m = maxBouwhoogte(pr);
        if (m === undefined) continue;
        doel = hoogte;
        extra = { meter: m, klasse: hoogteKlasse(m) };
      } else if (pr.soort === 'enkelbestemming') {
        const g = BESTEMMING.find((x) => x.hoofdgroepen.includes(pr.hoofdgroep));
        if (!g) continue; // TAM-locaties als "Gebouw bouwen" hebben geen functie (hoofdgroep overig)
        doel = bestemming;
        extra = { groep: g.groep };
      } else {
        doel = bouwvlak;
      }
      if (geldendPlan(pr.punt, index, lagen, cache) !== plan.id) continue;
      doel.push({ type: 'Feature', geometry: f.geometry, properties: { plan: plan.id, naam: pr.naam, ...extra } });
    }
  }

  return { hoogte: fc(hoogte), bestemming: fc(bestemming), bouwvlak: fc(bouwvlak) };
}

/** Raster over het zoekgebied: per cel de dekking van de toets in het midden van de cel. */
export function dekkingsraster({ index, lagen, zoekgebied }, cel = 20) {
  const vlak = zoekgebied.features[0].geometry;
  const rd = vlak.coordinates[0].map((ll) => naarRd(ll));
  const xs = rd.map((c) => c[0]);
  const ys = rd.map((c) => c[1]);
  const uit = [];
  for (let x = Math.floor(Math.min(...xs) / cel) * cel; x < Math.max(...xs); x += cel) {
    for (let y = Math.floor(Math.min(...ys) / cel) * cel; y < Math.max(...ys); y += cel) {
      const midden = vanRd([x + cel / 2, y + cel / 2]);
      if (!puntInVlak(midden, vlak)) continue;
      const d = dekkingOpPunt(bepaalStapel(midden, index, lagen));
      // Rekent = het geldende recht rekent: het tijdelijk deel, of een vastgesteld TAM-plan (dan is
      // het tijdelijk deel niet van toepassing). Een ontwerp-TAM (beoogd) telt niet mee.
      let klasse = null;
      if (d.tijdelijk === 'gerekend' || (d.nieuw === 'gerekend' && d.tijdelijk === 'niet-van-toepassing')) klasse = 'rekent';
      else if (d.tijdelijk === 'bron-ontbreekt') klasse = 'bron';
      else if (d.tijdelijk === 'niet-gemodelleerd' || d.nieuw === 'niet-gemodelleerd') klasse = 'open';
      else if (d.nieuw === 'gerekend') klasse = 'rekent';
      if (!klasse) continue;
      const hoek = [[x, y], [x + cel, y], [x + cel, y + cel], [x, y + cel], [x, y]].map(vanRd);
      uit.push({ type: 'Feature', geometry: { type: 'Polygon', coordinates: [hoek] }, properties: { klasse, ...d } });
    }
  }
  return uit;
}

/** Kleuren voor het thema Plan. De kleur scheidt buren; het nummer zegt welk plan het is. */
export const PLAN_KLEUREN = ['#2a78d6', '#e17000', '#1baf7a', '#6250d6', '#d52b1e', '#b38f00', '#0f8a9d', '#b0338f'];
const PLAN_ONDERLAAG = '#9db4c8';
const PLAN_NOOT = {
  ontwerp: 'ontwerp, geldt nog niet',
  laag: 'ligt over andere plannen heen',
  vervangen: 'in dit gebied vervangen door nieuwere plannen',
};

/** Naam van een plan zoals hij op de kaart past: zonder "Bestemmingsplan" en de plaatsnaam, TAM afgekort. */
export function planNaam(naam) {
  const kort = naam.replace(/^bestemmingsplan\s+/i, '').replace(/^TAM-omgevingsplan\s+/, 'TAM ').replace(/,? Veenendaal$/, '').trim();
  return kort.charAt(0).toUpperCase() + kort.slice(1);
}

/**
 * Thema Plan: welk plan ligt waar. Per plan het plangebied, een nummer en een plek voor dat nummer.
 * Een plan is "geldend" als het ergens in het zoekgebied het plan is dat de regels geeft (stapel.js):
 * een vastgesteld TAM-plan, anders het basisplan. Dat is gemeten op een raster en op het binnenpunt
 * van het plangebied. Het parapluplan ligt over de hele gemeente en staat er niet in.
 * Het nummer staat op een plek waar het plan zelf geldt, niet waar een nieuwer plan eroverheen ligt.
 */
export function planLaag({ index, lagen, zoekgebied }, cel = 20) {
  const vlak = zoekgebied.features[0].geometry;
  const rand = vlak.coordinates[0].map((ll) => naarRd(ll));
  const xs = rand.map((c) => c[0]);
  const ys = rand.map((c) => c[1]);
  const geldt = (ll) => { const s = bepaalStapel(ll, index, lagen); return (s.tam ?? s.basisplan)?.plan.id ?? null; };
  const cellen = new Map();
  for (let x = Math.floor(Math.min(...xs) / cel) * cel; x < Math.max(...xs); x += cel) {
    for (let y = Math.floor(Math.min(...ys) / cel) * cel; y < Math.max(...ys); y += cel) {
      const m = [x + cel / 2, y + cel / 2];
      const ll = vanRd(m);
      if (!puntInVlak(ll, vlak)) continue;
      const id = geldt(ll);
      if (!id) continue;
      if (!cellen.has(id)) cellen.set(id, []);
      cellen.get(id).push(m);
    }
  }

  const plannen = [];
  let k = 0;
  for (const plan of index.plannen.filter((p) => p.rol !== 'paraplu').sort((a, b) => a.datum.localeCompare(b.datum))) {
    const gebied = lagen.get(plan.id).features.filter((f) => f.properties.soort === 'plangebied');
    if (!gebied.length) continue;
    const binnenpunt = gebied[0].properties.punt;
    const c = cellen.get(plan.id);
    let soort = 'vervangen';
    if (plan.rol === 'tam-ontwerp') soort = 'ontwerp';
    else if (plan.rol === 'facet') soort = 'laag';
    else if (c?.length || (binnenpunt && geldt(binnenpunt) === plan.id)) soort = 'geldend';
    let punt = binnenpunt;
    if (soort === 'geldend' && c?.length) {
      // De cel die het dichtst bij het zwaartepunt van de eigen cellen ligt.
      const mx = c.reduce((s, p) => s + p[0], 0) / c.length;
      const my = c.reduce((s, p) => s + p[1], 0) / c.length;
      punt = vanRd(c.reduce((a, p) => (Math.hypot(p[0] - mx, p[1] - my) < Math.hypot(a[0] - mx, a[1] - my) ? p : a)));
    }
    const onderlaag = plan.id.includes('Woongeb');
    const kleur = soort !== 'geldend' ? '#4a4a4a' : onderlaag ? PLAN_ONDERLAAG : PLAN_KLEUREN[k++ % PLAN_KLEUREN.length];
    plannen.push({
      id: plan.id, naam: planNaam(plan.naam), soort, noot: PLAN_NOOT[soort] ?? null, kleur, licht: onderlaag && soort === 'geldend',
      punt, datum: plan.datum, ha: c?.length ? Math.round((c.length * cel * cel) / 1000) / 10 : null, gebied,
    });
  }
  // Nummers: eerst de geldende plannen op naam, daarna de rest.
  const rang = { geldend: 0, ontwerp: 1, laag: 2, vervangen: 3 };
  plannen.sort((a, b) => rang[a.soort] - rang[b.soort] || a.naam.localeCompare(b.naam, 'nl'));
  plannen.forEach((p, i) => { p.nr = i + 1; });
  // Tekenvolgorde: wat niet geldt onderop, daarna van oud naar nieuw, zodat het nieuwste plan bovenop ligt.
  const tekenen = [...plannen].sort((a, b) => (a.soort === 'geldend') - (b.soort === 'geldend') || a.datum.localeCompare(b.datum));
  const vlakken = tekenen.flatMap((p) => p.gebied.map((f) => ({
    type: 'Feature', geometry: f.geometry,
    properties: { plan: p.id, nr: p.nr, naam: p.naam, soort: p.soort, noot: p.noot, kleur: p.kleur },
  })));
  return { vlakken: fc(vlakken), plannen: plannen.map(({ gebied, ...rest }) => rest) };
}
