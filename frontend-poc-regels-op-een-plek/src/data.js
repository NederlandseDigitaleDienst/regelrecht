// Gegevens en vaste waarden van de demo. Leest de vooraf gerekende uitkomsten (demo.json in de
// bron); deze module rekent zelf niets.
import proj4 from 'proj4';
import { json } from './bron.js';

proj4.defs('EPSG:28992', '+proj=sterea +lat_0=52.15616055555555 +lon_0=5.38763888888889 +k=0.9999079 +x_0=155000 +y_0=463000 +ellps=bessel +towgs84=565.4171,50.3319,465.5524,1.9342,-1.6677,9.1019,4.0725 +units=m +no_defs');
export const naarLL = ([x, y]) => { const [lon, lat] = proj4('EPSG:28992', 'EPSG:4326', [x, y]); return [lat, lon]; };
export const naarRD = (lat, lon) => proj4('EPSG:4326', 'EPSG:28992', [lon, lat]);

let belofte = null;
export const laadDemo = () => (belofte ??= Promise.resolve().then(() => json('demo.json')));
let tekstBelofte = null;
export const laadTeksten = () => (tekstBelofte ??= Promise.resolve().then(() => json('teksten.json')));

// Kleuren van de uitkomst: dezelfde als de kaartlaag Uitkomst in de eerste PoC (kaart/uitkomst.js).
export const KLASSEN = [
  { klasse: 'toegestaan', tekst: 'Toegestaan', kleur: '#0ca30c' },
  { klasse: 'onthouding', tekst: 'Onthouding: er ontbreekt een gegeven', kleur: '#fab219' },
  { klasse: 'bron', tekst: 'Onthouding: regels alleen als scan (Het Ambacht 2009)', kleur: '#8c6d1f' },
  { klasse: 'niet', tekst: 'Niet toegestaan', kleur: '#d03b3b' },
  { klasse: 'open', tekst: 'Rekent hier niet: geen plan of geen regels', kleur: '#8a8a86' },
];
export const KLEUR = Object.fromEntries(KLASSEN.map((k) => [k.klasse, k.kleur]));

// Verschil tussen de sporen, in rijkshuisstijlkleuren.
export const VERSCHIL = [
  { klasse: 'gelijk', tekst: 'Zelfde uitkomst', kleur: '#b4b4b0' },
  { klasse: 'dsoOnthoudt', tekst: 'DSO-spoor onthoudt zich, eerste PoC beslist', kleur: '#fab219' },
  { klasse: 'dsoBeslist', tekst: 'DSO-spoor beslist, eerste PoC onthoudt zich', kleur: '#01689b' },
  { klasse: 'dsoRuimer', tekst: 'DSO-spoor zegt ja, eerste PoC nee', kleur: '#a90061' },
  { klasse: 'dsoStrenger', tekst: 'DSO-spoor zegt nee, eerste PoC ja', kleur: '#d03b3b' },
  { klasse: 'geenPlan', tekst: 'Plan niet in het DSO (Het Ambacht 2009)', kleur: '#8c6d1f' },
];

export const LAGEN = [
  { id: 'oud', kort: 'Vóór DSO', tekst: 'Vóór DSO: de eerste PoC', uitleg: 'Eigen downloads van ruimtelijkeplannen.nl, gelaagdheid en regels met de hand. Alleen de gemeente.' },
  { id: 'dso', kort: 'Met DSO', tekst: 'Met DSO: oordeel van de gemeente', uitleg: 'Plannen, regels en teksten uit het DSO; gelaagdheid afgeleid uit de gegevens; regels gegenereerd in vormen.' },
  { id: 'totaal', kort: 'Totaal', tekst: 'Met DSO: gemeente, provincie, waterschap en rijk', uitleg: 'Niet toegestaan zodra één overheid nee zegt. Een vergunningplicht is geen nee.' },
  { id: 'verschil', kort: 'Verschil', tekst: 'Verschil tussen de sporen (gemeente)', uitleg: 'Waar het DSO-spoor iets anders zegt dan de eerste PoC.' },
];

export const UIT_KLASSE = { toegestaan: 'toegestaan', 'niet toegestaan': 'niet', onthouding: 'onthouding' };
export const TAG = { toegestaan: 'success', niet: 'critical', 'niet toegestaan': 'critical', onthouding: 'warning', bron: 'warning', open: 'neutral', 'buiten bereik': 'neutral' };
export const TEKST = { toegestaan: 'toegestaan', niet: 'niet toegestaan', 'niet toegestaan': 'niet toegestaan', onthouding: 'onthouding', open: 'rekent hier niet', bron: 'alleen scan', 'buiten bereik': 'buiten bereik' };
export const BANNER = {
  toegestaan: { variant: 'success', tekst: 'Toegestaan' },
  niet: { variant: 'critical', tekst: 'Niet toegestaan' },
  onthouding: { variant: 'warning', tekst: 'Onthouding' },
  bron: { variant: 'warning', tekst: 'Onthouding: bron alleen als scan' },
  open: { variant: 'neutral', tekst: 'Rekent hier niet' },
};
export const OVERHEID = { gemeente: 'Gemeente Veenendaal', provincie: 'Provincie Utrecht', waterschap: 'Waterschap Vallei en Veluwe', rijk: 'Rijk', ministerie: 'Rijk' };
export const BUNDEL = { bouwen: 'gebouw bouwen', 'bouwen-technisch': 'technisch bouwen', wonen: 'wonen', 'woning-toevoegen': 'woningen toevoegen', funderen: 'funderen', bemalen: 'bemalen', aansluiten: 'riool', uitweg: 'uitrit' };

export const kortId = (id) => String(id ?? '').replace('NL.IMRO.0345.', '').replace('/akn/nl/act/', '');

export const klasseOud = (c, weergave) => (c.oud ? (weergave === 'beoogd' ? c.oud.beoogd : c.oud.geldend) : null);
export const klasseDso = (c, weergave) => (weergave === 'beoogd' ? c.dso.beoogd : c.dso.geldend);
export const klasseTotaal = (c, weergave) => {
  if (weergave === 'beoogd') return c.dso.beoogd;
  return c.dso.totaal ? UIT_KLASSE[c.dso.totaal] : 'open';
};
export function verschil(c, weergave) {
  const o = klasseOud(c, weergave); const d = klasseDso(c, weergave);
  if (o == null && d == null) return null;
  if (o === 'bron') return 'geenPlan';
  if (o === d) return 'gelijk';
  if (d === 'onthouding' || d === 'open') return 'dsoOnthoudt';
  if (o === 'onthouding' || o === 'open') return 'dsoBeslist';
  if (d === 'toegestaan') return 'dsoRuimer';
  return 'dsoStrenger';
}
export function klasse(c, laag, weergave) {
  if (laag === 'verschil') return verschil(c, weergave);
  if (laag === 'oud') return klasseOud(c, weergave);
  if (laag === 'dso') return klasseDso(c, weergave);
  return klasseTotaal(c, weergave);
}

// De twee reeksen scenario's.
export const REEKSEN = [
  { id: 'woontoren', kort: 'Woontoren', voorvoegsel: 'W', titel: "Zeventien blinde scenario's",
    uitleg: 'Geschreven door een aparte sessie die alleen de bronnen zag, vóór het modelleren. Bouwplan: 5 lagen van 3,2 m, 20 woningen. Verwachting uit toelichting of regeltekst, met citaat.' },
  { id: 'besluiten', kort: 'Besluiten', voorvoegsel: 'B', titel: "Negentien scenario's uit besluiten",
    uitleg: 'Verwachting uit echte besluiten van de gemeente (officielebekendmakingen.nl): verleend = toegestaan, BOPA (buitenplans afgeweken) = niet toegestaan, geweigerd = niet toegestaan. Bouwplan volgens het besluit; de hoogte is vaak een aanname.' },
];
