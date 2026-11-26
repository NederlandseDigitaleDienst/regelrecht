<script setup>
// De kaart met alles erop en eraan: plannen, kaartlagen, Op dit punt, Toets, Simuleren,
// Testpunten, Uitleg en de graaf. Die onderdelen staan in src/eerste (alias @eerste). Wat het
// DSO-spoor toevoegt, staat apart en is gemarkeerd met "DSO-spoor":
//   - de tab "Met en zonder DSO" (DsoTab.vue): beide sporen op het punt, simuleren met het
//     DSO-spoor, de scenario's en de kerncijfers;
//   - het kaartthema DSO: vóór DSO, met DSO, alle overheden, het verschil en de simulatie, per
//     vak van 40 m, met de scenario's als genummerde punten.
import 'leaflet/dist/leaflet.css';
import L from 'leaflet';
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue';
import FeitenPaneel from '@eerste/components/FeitenPaneel.vue';
import ToetsTab from '@eerste/components/ToetsTab.vue';
import SimulerenTab from '@eerste/components/SimulerenTab.vue';
import GraafTab from '@eerste/components/GraafTab.vue';
import UitlegTab from '@eerste/components/UitlegTab.vue';
import { json } from '../bron.js';
import { laadKaartdata } from '@eerste/kaart/laden.js';
import { naarRd, objectenOpPunt, vanRd } from '@eerste/kaart/geo.js';
import { bepaalStapel } from '@eerste/kaart/stapel.js';
import { BESTEMMING, DEKKING, HOOGTE, bouwContext, dekkingsraster, planLaag } from '@eerste/kaart/context.js';
import { UITKOMST_KLASSEN, rasterCellen, rekenUitkomst, verfijnCellen } from '@eerste/kaart/uitkomst.js';
import { maakEngine } from '@eerste/engine.js';
import { bouwplan } from '@eerste/toets/bouwplan.js';
import { VELDEN, aannames, aantalIngevuld, naarParams } from '@eerste/toets/aannames.js';
import { laadRegelingen } from '@eerste/toets/laden.js';
import { bewaarEngines } from '@eerste/toets/simuleer.js';
// DSO-spoor
import DsoTab from './DsoTab.vue';
import ArtikelSheet from './ArtikelSheet.vue';
import { KLASSEN, KLEUR as DSO_KLEUR, LAGEN, REEKSEN, VERSCHIL, klasse as dsoKlasse, laadDemo } from '../data.js';
import { aannames as dsoAannames, hoogte, isStandaard, laadMotor, rekenKaart } from '../sim.js';

const kaartEl = ref(null);
const paneelEl = ref(null);
const fout = ref(null);
const stapel = shallowRef(null);
const rd = ref(null);
// DSO-spoor: ?tab=dso opent direct de tab Met en zonder DSO.
const tab = ref(new URLSearchParams(window.location.search).get('tab') === 'dso' ? 'dso' : 'punt');
const testpunten = ref([]);
const gekozenTestpunt = ref(null);
// Kaart of graaf in de grote ruimte. De kaart blijft eronder staan.
const weergave = ref(new URLSearchParams(window.location.search).get('weergave') === 'graaf' ? 'graaf' : 'kaart');
function kiesWeergave(w) {
  weergave.value = w;
  const url = new URL(window.location.href);
  if (w === 'graaf') url.searchParams.set('weergave', 'graaf'); else url.searchParams.delete('weergave');
  window.history.replaceState(null, '', url);
  if (w === 'kaart') requestAnimationFrame(() => kaart?.invalidateSize());
}
let kaart = null;
let data = null;
let marker = null;
let accent = null;

// Rijkshuisstijlkleuren voor de lijnen op de kaart (Leaflet tekent zelf, buiten de componenten).
const KLEUR = {
  basisplan: '#01689b', wijzigingsplan: '#01689b', uitwerkingsplan: '#01689b', // lintblauw
  facet: '#d52b1e', // rood
  tam: '#275937', 'tam-ontwerp': '#275937', // donkergroen
};
const ZOEKGEBIED = '#154273'; // donkerblauw
const ACCENT = '#e17000'; // oranje

// Legenda in groepen. Vorm: lijn (grens), vlak (plangebied), punt, nummer (testpunt).
const LEGENDA = [
  { kop: 'Gebied', items: [
    { tekst: 'Zoekgebied (afgeleid, geen officiële grens)', vorm: 'lijn', kleur: ZOEKGEBIED, stippel: true },
  ] },
  { kop: 'Plannen', items: [
    { tekst: 'Bestemmingsplan', vorm: 'vlak', kleur: KLEUR.basisplan },
    { tekst: 'TAM-plan, vastgesteld', vorm: 'vlak', kleur: KLEUR.tam },
    { tekst: 'TAM-plan, ontwerp', vorm: 'vlak', kleur: KLEUR['tam-ontwerp'], stippel: true },
    { tekst: 'Ondergrondse hoogspanning', vorm: 'vlak', kleur: KLEUR.facet },
  ] },
  { kop: 'Op het gekozen punt', items: [
    { tekst: 'Gekozen punt', vorm: 'punt' },
    { tekst: 'Bestemming of locatie', vorm: 'vlak', kleur: ACCENT },
    { tekst: 'Bouwvlak', vorm: 'vlak', kleur: '#000', stippel: true },
    { tekst: 'Testpunt', vorm: 'nummer' },
  ] },
];
// Contextlagen voor woningbouw (context.js): één thema tegelijk, plus de bouwvlakken.
const THEMA = [
  { id: 'plan', kort: 'Plan', tekst: 'Welk plan ligt waar', uitleg: 'Het plan dat op een plek de regels geeft. Het nieuwste plan ligt bovenop. Grote plannen staan met naam op de kaart, kleine met een nummer. Wijs een vlak aan voor de naam.', items: [] },
  { id: 'hoogte', kort: 'Hoogte', tekst: 'Maximale bouwhoogte', uitleg: 'Maatvoering op de plankaart van het plan dat hier geldt.',
    items: HOOGTE.map((k) => ({ tekst: k.tekst, kleur: k.kleur })) },
  { id: 'bestemming', kort: 'Functie', tekst: 'Bestemming', uitleg: 'Hoofdgroep uit de plankaart van het plan dat hier geldt.',
    items: BESTEMMING.map((k) => ({ tekst: k.tekst, kleur: k.kleur })) },
  { id: 'dekking', kort: 'Toets', tekst: 'Dekking van de toets', uitleg: 'Waar de toets nu echt rekent. Raster van 20 meter.',
    items: DEKKING.map((k) => ({ tekst: k.tekst, kleur: k.kleur })) },
  { id: 'uitkomst', kort: 'Uitkomst', tekst: 'Uitkomst van de toets', uitleg: 'Met het bouwplan en de aannames uit Simuleren. Raster van 40 meter, 10 meter waar de regels binnen een vak verschillen. Aantallen in vakken van 40 meter.',
    items: UITKOMST_KLASSEN.map((k) => ({ tekst: k.tekst, kleur: k.kleur, klasse: k.klasse })) },
  // DSO-spoor: legenda en bediening staan in een eigen blok in de legenda.
  { id: 'dso', kort: 'DSO', tekst: 'Met en zonder DSO', uitleg: '', items: [] },
];
// Deellink: ?thema=plan opent de kaart direct met dat thema.
const urlParams = new URLSearchParams(window.location.search);
const themaUitUrl = urlParams.get('thema') ?? (urlParams.get('tab') === 'dso' ? 'dso' : null);
const thema = ref(THEMA.some((t) => t.id === themaUitUrl) ? themaUitUrl : 'hoogte');
const toonBouwvlak = ref(true);
const themaNu = computed(() => THEMA.find((t) => t.id === thema.value));
const contextLagen = {};
// Thema Plan: de lijst voor de legenda, en per plan zijn vlakken op de kaart.
const PLAN_NAAM_VANAF_HA = 1;
const planLijst = ref([]);
const planVlak = new Map();
let planVlakken = null;
let context = null;
let canvas = null;

/** Bouw de contextlagen pas als ze nodig zijn; de dekking kost het meeste rekenwerk. */
function contextLaag(id) {
  if (contextLagen[id]) return contextLagen[id];
  context ??= bouwContext(data);
  const opties = { renderer: canvas, interactive: false };
  if (id === 'hoogte') {
    contextLagen[id] = L.geoJSON(context.hoogte, { ...opties, style: (f) => ({ color: HOOGTE[f.properties.klasse].kleur, weight: 0.6, fillColor: HOOGTE[f.properties.klasse].kleur, fillOpacity: 0.72 }) });
  } else if (id === 'bestemming') {
    const kleur = Object.fromEntries(BESTEMMING.map((k) => [k.groep, k.kleur]));
    contextLagen[id] = L.geoJSON(context.bestemming, { ...opties, style: (f) => ({ color: kleur[f.properties.groep], weight: 0.4, fillColor: kleur[f.properties.groep], fillOpacity: 0.6 }) });
  } else if (id === 'dekking') {
    const kleur = Object.fromEntries(DEKKING.map((k) => [k.klasse, k.kleur]));
    contextLagen[id] = L.geoJSON({ type: 'FeatureCollection', features: dekkingsraster(data) }, { ...opties, style: (f) => ({ stroke: false, fillColor: kleur[f.properties.klasse], fillOpacity: 0.38 }) });
  } else if (id === 'plan') {
    // Eigen SVG-laag in plaats van canvas: de vlakken tonen bij aanwijzen de naam van het plan.
    const pl = planLaag(data);
    planLijst.value = pl.plannen;
    planVlakken = L.geoJSON(pl.vlakken, {
      renderer: L.svg({ pane: 'context' }),
      style: (f) => {
        const vol = f.properties.soort === 'geldend';
        return { color: f.properties.kleur, weight: vol ? 1.5 : 2, opacity: 0.9, dashArray: vol ? null : '6 4', fillColor: f.properties.kleur, fillOpacity: vol ? 0.42 : 0 };
      },
      onEachFeature: (f, laag) => {
        const pr = f.properties;
        laag.bindTooltip(`<strong>${pr.nr}. ${pr.naam}</strong>${pr.noot ? `<br>${pr.noot}` : ''}`, { sticky: true, direction: 'top' });
        laag.on('mouseover', () => laag.setStyle({ weight: 3.5 }));
        laag.on('mouseout', () => planVlakken.resetStyle(laag));
        if (!planVlak.has(pr.plan)) planVlak.set(pr.plan, []);
        planVlak.get(pr.plan).push(laag);
      },
    });
    const nummers = pl.plannen.map((pn) => L.marker([pn.punt[1], pn.punt[0]], {
      // Plannen vanaf 1 hectare krijgen hun naam op de kaart; de kleinere alleen een nummer.
      icon: L.divIcon({ className: 'plan-nr', html: `<span style="background:${pn.kleur};color:${pn.licht ? '#1b1b1b' : '#fff'}">${pn.ha >= PLAN_NAAM_VANAF_HA ? `${pn.nr} · ${pn.naam}` : pn.nr}</span>`, iconSize: [0, 0] }),
      interactive: false, keyboard: false,
    }));
    contextLagen[id] = L.layerGroup([planVlakken, ...nummers]);
  } else if (id === 'bouwvlak') {
    contextLagen[id] = L.geoJSON(context.bouwvlak, { ...opties, style: { color: '#1b1b1b', weight: 1, opacity: 0.7, fill: false } });
  } else if (id === 'dso') {
    contextLagen[id] = dsoLaagBouwen();
  }
  return contextLagen[id];
}

function toonContext() {
  if (!kaart || !data) return;
  for (const [id, laag] of Object.entries(contextLagen)) {
    const aan = id === 'bouwvlak' ? toonBouwvlak.value : id === thema.value;
    if (laag && !aan && kaart.hasLayer(laag)) laag.remove();
  }
  if (thema.value === 'uitkomst') rekenUitkomstLaag();
  else contextLaag(thema.value)?.addTo(kaart);
  if (toonBouwvlak.value) contextLaag('bouwvlak').addTo(kaart).bringToFront();
  // DSO-spoor: de scenario's als genummerde punten, alleen bij het thema DSO.
  if (thema.value === 'dso') pins.addTo(kaart); else pins.remove();
}
watch([thema, toonBouwvlak], toonContext);

/** Zoom naar een plan uit de legenda en licht de grens even op. */
function zoomNaarPlan(pn) {
  const vlakken = planVlak.get(pn.id);
  if (!vlakken?.length) return;
  const grenzen = vlakken.reduce((g, v) => g.extend(v.getBounds()), L.latLngBounds([]));
  kaart.fitBounds(grenzen, { paddingBottomRight: [paneelBreedte() + 24, 24], paddingTopLeft: [24, 24], maxZoom: 18 });
  for (const v of vlakken) v.setStyle({ weight: 4.5 });
  setTimeout(() => { for (const v of vlakken) planVlakken.resetStyle(v); }, 1600);
}

// --- Kaartlaag Uitkomst: de toets per cel, opnieuw bij elke wijziging van bouwplan of aannames.
const uitkomstWeergave = ref('geldend');
const uitkomstVoortgang = ref(null); // 0 tot 1 tijdens het rekenen, anders null
const uitkomstTelling = ref(null);
const uitkomstFout = ref(null);
const uitkomstOntbreekt = ref(null);
// Waarom de cellen met "er ontbreekt een gegeven" onthouding zijn, meest voorkomend eerst.
const ONTBREEKT_TEKST = {
  kaartfeit: 'Maat die niet op de kaart staat (niet in te vullen)',
  'niet-in-yaml': 'Regel nog niet in YAML (niet in te vullen)',
};
const ontbreektLijst = computed(() => Object.entries(uitkomstOntbreekt.value ?? {})
  .map(([id, aantal]) => {
    const v = VELDEN.find((x) => x.id === id);
    return { id, aantal, tekst: v ? v.tekst : ONTBREEKT_TEKST[id] ?? id, invulbaar: !!v };
  })
  .sort((a, b) => b.aantal - a.aantal));
const ingevuld = computed(() => aantalIngevuld());
const bouwplanTekst = computed(() => `${bouwplan.lagen} lagen ${bouwplan.woningtype}, ${String(Math.round(bouwplan.lagen * bouwplan.meterPerLaag * 100) / 100).replace('.', ',')} m`);
let cellen = null;
let engines = null;
let ronde = 0;

async function rekenUitkomstLaag() {
  const deze = ++ronde;
  uitkomstVoortgang.value = 0;
  try {
    // Eerst de voortgangsbalk tonen; het raster opbouwen kost de eerste keer een paar seconden.
    if (!cellen) await new Promise((r) => setTimeout(r, 30));
    cellen ??= verfijnCellen(data, rasterCellen(data));
    engines ??= bewaarEngines(maakEngine);
    const { index, varianten } = await laadRegelingen();
    const uit = await rekenUitkomst({
      cellen, bouwplan: { ...bouwplan }, aannames: naarParams(), weergave: uitkomstWeergave.value,
      datum: new Date().toISOString().slice(0, 10), index, varianten, maakEngine: engines,
      opVoortgang: (f) => { if (deze === ronde) uitkomstVoortgang.value = f; }, gestopt: () => deze !== ronde,
    });
    if (!uit || deze !== ronde) return;
    contextLagen.uitkomst?.remove();
    const kleur = Object.fromEntries(UITKOMST_KLASSEN.map((k) => [k.klasse, k.kleur]));
    contextLagen.uitkomst = L.geoJSON({ type: 'FeatureCollection', features: uit.features }, {
      renderer: canvas, interactive: false,
      style: (f) => ({ stroke: false, fillColor: kleur[f.properties.klasse], fillOpacity: f.properties.klasse === 'open' ? 0.22 : 0.5 }),
    });
    if (thema.value === 'uitkomst') {
      contextLagen.uitkomst.addTo(kaart);
      if (toonBouwvlak.value && contextLagen.bouwvlak) contextLagen.bouwvlak.bringToFront();
    }
    uitkomstTelling.value = uit.telling;
    uitkomstOntbreekt.value = uit.ontbreekt;
    uitkomstFout.value = null;
  } catch (e) {
    uitkomstFout.value = String(e?.message ?? e);
  } finally {
    if (deze === ronde) uitkomstVoortgang.value = null;
  }
}
// Wijzigt het bouwplan of een aanname: opnieuw rekenen als de laag aan staat, anders bij het aanzetten.
let wacht = null;
watch([() => ({ ...bouwplan }), () => ({ ...aannames }), uitkomstWeergave], () => {
  if (thema.value !== 'uitkomst') { ronde++; contextLagen.uitkomst?.remove(); delete contextLagen.uitkomst; uitkomstTelling.value = null; return; }
  clearTimeout(wacht);
  wacht = setTimeout(rekenUitkomstLaag, 350);
}, { deep: true });

// Simuleren, deel "Gegevens invullen", en de sprong naar de uitleg.
const simDeel = ref('lagen');
function toonUitkomstOpKaart() {
  thema.value = 'uitkomst';
  if (weergave.value !== 'kaart') kiesWeergave('kaart');
}
function naarUitleg(begrip) {
  kiesTab('uitleg');
  requestAnimationFrame(() => document.getElementById(`uitleg-${begrip}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' }));
}
function sluitUitleg() {
  if (tab.value === 'uitleg') kiesTab(vorigeTab.value);
}
function wisselUitleg() {
  if (tab.value === 'uitleg') sluitUitleg(); else kiesTab('uitleg');
}
// Escape sluit de uitleg.
function opToets(e) {
  if (e.key === 'Escape' && tab.value === 'uitleg') sluitUitleg();
}
window.addEventListener('keydown', opToets);
onBeforeUnmount(() => window.removeEventListener('keydown', opToets));
function naarInvoer() {
  simDeel.value = 'invoer';
  kiesTab('simuleren');
}

// --- DSO-spoor: de vooraf gerekende uitkomst van beide sporen per vak van 40 m (public/demo.json),
// de simulatie met het DSO-spoor, en de scenario's.
const dso = shallowRef(null);
const dsoFout = ref(null);
const DSO_DELEN = ['punt', 'simuleren', 'scenarios', 'cijfers'];
const dsoDeel = ref(DSO_DELEN.includes(urlParams.get('deel')) ? urlParams.get('deel') : 'punt');
const dsoLaag = ref([...LAGEN.map((l) => l.id), 'sim'].includes(urlParams.get('laag')) ? urlParams.get('laag') : 'verschil');
const dsoReeks = ref(REEKSEN.some((r) => r.id === urlParams.get('reeks')) ? urlParams.get('reeks') : 'woontoren');
const dsoCel = shallowRef(null);
const dsoScenario = shallowRef(null);
const artikel = ref(null);
const dsoIndex = new Map();
const dsoCelIndex = computed(() => (dsoCel.value ? dsoIndex.get(dsoCel.value) : -1));
const pins = L.layerGroup();

const SIM_LAAG = { id: 'sim', kort: 'Simulatie', tekst: 'Simulatie: met DSO, oordeel van de gemeente' };
const simUitkomst = shallowRef(null);
const simVoortgang = ref(null);
const simFout = ref(null);
const simTekst = computed(() => `${bouwplan.lagen} lagen ${bouwplan.woningtype}, ${String(hoogte()).replace('.', ',')} m${isStandaard() ? ', standaard' : ''}${dsoAannames.gegeven !== 'gunstig' ? `, overige gegevens ${dsoAannames.gegeven}` : ''}`);
const dsoLagen = computed(() => (simUitkomst.value || dsoLaag.value === 'sim' ? [...LAGEN, SIM_LAAG] : LAGEN));
const dsoLaagNu = computed(() => (dsoLaag.value === 'sim'
  ? { ...SIM_LAAG, uitleg: 'Opnieuw gerekend in je browser, met het bouwplan en de aannames uit Simuleren onder Met en zonder DSO.' }
  : LAGEN.find((l) => l.id === dsoLaag.value)));

function dsoKlasseVan(c) {
  if (dsoLaag.value !== 'sim') return dsoKlasse(c, dsoLaag.value, uitkomstWeergave.value);
  const r = simUitkomst.value?.[dsoIndex.get(c)];
  if (!r) return null;
  return uitkomstWeergave.value === 'beoogd' ? r.beoogd : r.geldend;
}
const dsoTelling = computed(() => {
  const t = {};
  for (const c of dso.value?.cellen ?? []) {
    if (c.extra) continue;
    const k = dsoKlasseVan(c);
    if (k) t[k] = (t[k] ?? 0) + 1;
  }
  return t;
});
const dsoLegenda = computed(() => (dsoLaag.value === 'verschil' ? VERSCHIL : KLASSEN.filter((k) => dsoTelling.value[k.klasse])));

/** De vakken van 40 m in de gekozen laag, op het canvas van de contextlagen. */
function dsoLaagBouwen() {
  if (!dso.value) return null;
  const kleuren = dsoLaag.value === 'verschil' ? Object.fromEntries(VERSCHIL.map((v) => [v.klasse, v.kleur])) : DSO_KLEUR;
  const features = [];
  for (const c of dso.value.cellen) {
    if (c.extra) continue;
    const k = dsoKlasseVan(c);
    if (!k) continue;
    const [x, y] = c.rd;
    const ring = [[x - 20, y - 20], [x + 20, y - 20], [x + 20, y + 20], [x - 20, y + 20], [x - 20, y - 20]].map(vanRd);
    features.push({ type: 'Feature', properties: { k }, geometry: { type: 'Polygon', coordinates: [ring] } });
  }
  return L.geoJSON({ type: 'FeatureCollection', features }, {
    renderer: canvas, interactive: false,
    style: (f) => ({ stroke: false, fillColor: kleuren[f.properties.k], fillOpacity: f.properties.k === 'open' || f.properties.k === 'gelijk' ? 0.28 : 0.5 }),
  });
}
function hertekenDso() {
  contextLagen.dso?.remove();
  delete contextLagen.dso;
  if (thema.value === 'dso') toonContext();
}
watch([dsoLaag, uitkomstWeergave, simUitkomst], hertekenDso);

let simRonde = 0;
async function rekenSimLaag() {
  if (!dso.value) return;
  const deze = ++simRonde;
  simVoortgang.value = 0;
  try {
    const m = await laadMotor();
    const uit = await rekenKaart(m, dso.value.cellen, { opVoortgang: (f) => { if (deze === simRonde) simVoortgang.value = f; }, gestopt: () => deze !== simRonde });
    if (!uit || deze !== simRonde) return;
    simUitkomst.value = uit;
    simFout.value = null;
  } catch (e) {
    simFout.value = String(e?.message ?? e);
  } finally {
    if (deze === simRonde) simVoortgang.value = null;
  }
}
function toonSimOpKaart() {
  thema.value = 'dso';
  dsoLaag.value = 'sim';
  if (weergave.value !== 'kaart') kiesWeergave('kaart');
  rekenSimLaag();
}
// Wijzigt het bouwplan of een aanname: opnieuw rekenen als de laag aan staat, anders bij het aanzetten.
let simWacht = null;
watch([() => ({ ...bouwplan }), () => ({ ...dsoAannames })], () => {
  if (dsoLaag.value !== 'sim' || thema.value !== 'dso') { simRonde++; simUitkomst.value = null; simVoortgang.value = null; return; }
  clearTimeout(simWacht);
  simWacht = setTimeout(rekenSimLaag, 350);
}, { deep: true });
watch([dsoLaag, thema], ([l, t]) => { if (l === 'sim' && t === 'dso' && !simUitkomst.value && simVoortgang.value === null) rekenSimLaag(); });

const scenarioLijst = computed(() => (dso.value ? (dsoReeks.value === 'besluiten' ? dso.value.besluiten : dso.value.scenarios) ?? [] : []));
function tekenPins() {
  pins.clearLayers();
  const voor = REEKSEN.find((r) => r.id === dsoReeks.value).voorvoegsel;
  scenarioLijst.value.forEach((r, i) => {
    const ll = vanRd(r.rd);
    L.marker([ll[1], ll[0]], { icon: L.divIcon({ className: 'tp-icoon sc-icoon', html: `<span>${voor}${i + 1}</span>`, iconSize: [30, 20], iconAnchor: [15, 10] }), title: r.naam, keyboard: true })
      .on('click', (e) => { L.DomEvent.stopPropagation(e); kiesScenario(r); })
      .addTo(pins);
  });
}
watch(dsoReeks, tekenPins);

/** Het vak van 40 m het dichtst bij een punt in RD, of null buiten het raster. */
function dsoVakBij([x, y]) {
  let best = null; let bd = Infinity;
  for (const c of dso.value?.cellen ?? []) {
    if (c.extra) continue;
    const dd = (c.rd[0] - x) ** 2 + (c.rd[1] - y) ** 2;
    if (dd < bd) { bd = dd; best = c; }
  }
  return best && bd < 40 * 40 ? best : null;
}
function kiesScenario(r) {
  if (r.recht === 'beoogde') uitkomstWeergave.value = 'beoogd';
  else if (uitkomstWeergave.value === 'beoogd') uitkomstWeergave.value = 'geldend';
  gekozenTestpunt.value = null;
  kiesRd(r.rd, 18);
  // Het scenario heeft een eigen vak, precies op zijn punt (data/extra-punten.json).
  const c = dso.value.cellen.find((x) => x.extra && x.rd[0] === r.rd[0] && x.rd[1] === r.rd[1]);
  if (c) { dsoCel.value = c; dsoScenario.value = { ...r, reeks: dsoReeks.value }; }
  dsoDeel.value = 'punt';
  if (tab.value !== 'dso') kiesTab('dso');
}
function zetDsoUrl() {
  const url = new URL(window.location.href);
  const zet = (k, v, standaard) => { if (v && v !== standaard) url.searchParams.set(k, v); else url.searchParams.delete(k); };
  zet('tab', tab.value === 'dso' ? 'dso' : null);
  zet('deel', tab.value === 'dso' ? dsoDeel.value : null, 'punt');
  zet('laag', thema.value === 'dso' ? dsoLaag.value : null, 'verschil');
  zet('reeks', dsoReeks.value, 'woontoren');
  zet('thema', thema.value, 'hoogte');
  window.history.replaceState(null, '', url);
}

const BREED = new Set(['toets', 'simuleren', 'uitleg', 'dso']);
const paneelBreed = computed(() => BREED.has(tab.value));

// Paneel minimaliseren: klein rechtsboven, met een knop om het weer
// te openen en per tab een knop om er direct heen te gaan. De inhoud blijft bewaard.
const TABS = [
  { id: 'punt', tekst: 'Op dit punt', icoon: 'map-pin' },
  { id: 'toets', tekst: 'Toets', icoon: 'checklist' },
  { id: 'simuleren', tekst: 'Simuleren', icoon: 'slider-horizontal-3' },
  { id: 'testpunten', tekst: 'Testpunten', icoon: 'list' },
  { id: 'dso', tekst: 'Met en zonder DSO', icoon: 'columns-2' }, // DSO-spoor
  { id: 'uitleg', tekst: 'Uitleg', icoon: 'book' },
];
const leesKlein = () => { try { return localStorage.getItem('spoorzone-paneel-klein') === '1'; } catch { return false; } };
const paneelKlein = ref(leesKlein());
function zetKlein(k) {
  paneelKlein.value = k;
  try { localStorage.setItem('spoorzone-paneel-klein', k ? '1' : '0'); } catch { /* geen opslag: alleen deze sessie */ }
}
function openPaneel(t) {
  zetKlein(false);
  if (t && t !== tab.value) kiesTab(t);
}
const tabNu = computed(() => TABS.find((t) => t.id === tab.value));

function kies(lonlat) {
  if (!data) return;
  const s = bepaalStapel(lonlat, data.index, data.lagen);
  stapel.value = s;
  rd.value = naarRd(lonlat);
  // DSO-spoor: het vak van 40 m onder het punt, voor de tab Met en zonder DSO.
  dsoCel.value = dsoVakBij(rd.value);
  dsoScenario.value = null;
  // In Toets of Simuleren blijven: die rekenen meteen op het nieuwe punt.
  if (!BREED.has(tab.value)) tab.value = 'punt';
  const url = new URL(window.location.href);
  url.searchParams.set('rd', `${Math.round(rd.value[0])},${Math.round(rd.value[1])}`);
  window.history.replaceState(null, '', url);
  const ll = [lonlat[1], lonlat[0]];
  if (marker) marker.setLatLng(ll);
  else marker = L.circleMarker(ll, { radius: 7, color: '#000', weight: 2, fillColor: '#ffb612', fillOpacity: 1 }).addTo(kaart);
  if (accent) accent.remove();
  const geldend = s.tam ?? s.basisplan;
  if (geldend) {
    // De vlakken die de feiten leveren: de bestemming en het bouwvlak op het punt.
    const raak = objectenOpPunt(data.lagen.get(geldend.plan.id), lonlat)
      .filter((f) => ['enkelbestemming', 'bouwvlak'].includes(f.properties.soort));
    accent = L.geoJSON({ type: 'FeatureCollection', features: raak }, {
      style: (f) => (f.properties.soort === 'bouwvlak'
        ? { color: '#000', weight: 1.5, dashArray: '4 3', fillOpacity: 0.05 }
        : { color: ACCENT, weight: 2, fillOpacity: 0.08 }),
      interactive: false,
    }).addTo(kaart);
  }
}

function kiesRd(xy, zoom = 17) {
  const lonlat = vanRd(xy);
  kaart.setView([lonlat[1], lonlat[0]], zoom, { animate: false });
  // Schuif de plek naar het midden van het zichtbare deel, links van het paneel.
  kaart.panBy([paneelBreedte() / 2, 0], { animate: false });
  kies(lonlat);
}

/** Breedte van het paneel over de kaart, om de kaart ernaast te centreren. */
function paneelBreedte() {
  return paneelEl.value?.getBoundingClientRect().width ?? 0;
}

function kiesTestpunt(t) {
  gekozenTestpunt.value = t.naam;
  kiesRd(t.rd);
}

// Waar je was voordat je de uitleg opende; het kruisje brengt je daar terug.
const vorigeTab = ref('punt');
function kiesTab(t) {
  if (t === 'uitleg' && tab.value !== 'uitleg') vorigeTab.value = tab.value;
  // DSO-spoor: wie de tab Met en zonder DSO opent, ziet op de kaart het thema DSO.
  if (t === 'dso' && tab.value !== 'dso') thema.value = 'dso';
  tab.value = t;
  // Het paneel verandert van breedte; laat Leaflet de nieuwe maat zien.
  requestAnimationFrame(() => kaart?.invalidateSize());
}

function onKaartKlik(e) {
  gekozenTestpunt.value = null;
  kies([e.latlng.lng, e.latlng.lat]);
}

onMounted(async () => {
  kaart = L.map(kaartEl.value, { zoomControl: true, attributionControl: false }).setView([52.022, 5.555], 15);
  // Bronvermelding linksonder: rechts ligt het paneel over de kaart.
  L.control.attribution({ position: 'bottomleft', prefix: false }).addTo(kaart);
  L.tileLayer('https://service.pdok.nl/brt/achtergrondkaart/wmts/v2_0/grijs/EPSG:3857/{z}/{x}/{y}.png', {
    maxZoom: 19, attribution: 'Kaart: <a href="https://www.pdok.nl">PDOK</a>, BRT Achtergrondkaart grijs (Kadaster)',
  }).addTo(kaart);
  // De kaart staat in een paneel dat pas na de eerste layout zijn maat kent.
  new ResizeObserver(() => kaart?.invalidateSize()).observe(kaartEl.value);
  try {
    const [kaartdata, tp, demo] = await Promise.all([
      laadKaartdata(),
      Promise.resolve().then(() => json('data/testpunten.json')).catch(() => ({ punten: [] })),
      // DSO-spoor: zonder deze gegevens werkt de rest gewoon; de tab meldt het.
      laadDemo().catch((e) => { dsoFout.value = String(e?.message ?? e); return null; }),
    ]);
    data = kaartdata;
    testpunten.value = tp.punten ?? [];
    if (demo) {
      demo.cellen.forEach((c, i) => dsoIndex.set(c, i));
      dso.value = demo;
    }
  } catch (e) {
    fout.value = String(e?.message ?? e);
    return;
  }
  // De gegevens staan al klaar (bron.js), dus dit kan binnen dezelfde frame als het mounten lopen,
  // voordat de kaart zijn maat kent. Eerst de layout afwachten, anders past fitBounds hieronder
  // het zoekgebied in een kaart van nul pixels en zoomt hij maximaal in.
  await new Promise((klaar) => requestAnimationFrame(() => klaar()));
  kaart.invalidateSize();
  // Contextlagen onder de plangrenzen, in een eigen laag met canvas: duizenden vlakken.
  kaart.createPane('context').style.zIndex = 350;
  canvas = L.canvas({ pane: 'context', padding: 0.2 });
  const zoek = L.geoJSON(data.zoekgebied, { style: { color: ZOEKGEBIED, weight: 2, dashArray: '6 4', fill: false }, interactive: false }).addTo(kaart);
  for (const plan of data.index.plannen) {
    if (plan.rol === 'paraplu') continue; // ligt over de hele gemeente; tekenen voegt niets toe
    const pg = data.lagen.get(plan.id).features.filter((f) => f.properties.soort === 'plangebied');
    const groot = plan.id.includes('Woongeb');
    L.geoJSON({ type: 'FeatureCollection', features: pg }, {
      style: {
        color: KLEUR[plan.rol] ?? '#666', weight: groot ? 0.5 : 1.5,
        dashArray: plan.rol === 'tam-ontwerp' ? '5 4' : null, fillOpacity: groot ? 0.03 : 0.07,
      },
      interactive: false,
    }).addTo(kaart);
  }
  // Testpunten voor de toets als genummerde punten op de kaart.
  for (const t of testpunten.value.filter((x) => x.toets)) {
    const ll = vanRd(t.rd);
    const label = t.naam.split(' ')[0];
    L.marker([ll[1], ll[0]], { icon: L.divIcon({ className: 'tp-icoon', html: `<span>${label}</span>`, iconSize: [28, 20], iconAnchor: [14, 10] }), title: t.naam, keyboard: true })
      .on('click', (e) => { L.DomEvent.stopPropagation(e); kiesTestpunt(t); })
      .addTo(kaart);
  }
  // Zonder animatie: een geanimeerde fitBounds overschrijft anders de deellink ?rd= hieronder.
  kaart.fitBounds(zoek.getBounds(), { paddingBottomRight: [paneelBreedte() + 24, 0], paddingTopLeft: [24, 24], animate: false });
  kaart.on('click', onKaartKlik);
  // DSO-spoor: de scenario's, en de simulatie als de deellink daarom vraagt (?thema=dso&laag=sim).
  tekenPins();
  if (thema.value === 'dso' && dsoLaag.value === 'sim') rekenSimLaag();
  // Na de eerste weergave, zodat de kaart niet wacht op het opbouwen van de lagen.
  setTimeout(toonContext, 0);
  // Deellink: ?rd=166117,447996 opent direct op dat punt.
  const xy = new URLSearchParams(window.location.search).get('rd')?.split(',').map(Number);
  if (xy?.length === 2 && xy.every(Number.isFinite)) kiesRd(xy);
  watch([tab, dsoDeel, dsoLaag, dsoReeks, thema], zetDsoUrl); // DSO-spoor
});

onBeforeUnmount(() => { kaart?.remove(); });
</script>

<template>
  <div class="kaarttab">
    <div class="kaartkader">
      <div ref="kaartEl" class="kaart" role="application" aria-label="Kaart van de Spoorzone. Klik op een plek om te zien welke regels daar gelden."></div>

      <nldd-segmented-control class="weergave-wissel" size="sm" :value="weergave" accessible-label="Weergave" @change="kiesWeergave($event.target.value)">
        <nldd-segmented-control-item value="kaart" text="Kaart" icon="map" :selected="weergave === 'kaart' || undefined"></nldd-segmented-control-item>
        <nldd-segmented-control-item value="graaf" text="Graaf" icon="centralized-network" :selected="weergave === 'graaf' || undefined"></nldd-segmented-control-item>
      </nldd-segmented-control>

      <GraafTab v-if="weergave === 'graaf'" :stapel="stapel" :rd="rd" class="graaflaag" />

      <div v-show="weergave === 'kaart'" class="legenda" aria-label="Kaartlagen en legenda">
        <nldd-text size="sm"><p><strong>Kaartlagen</strong></p></nldd-text>
        <nldd-segmented-control :value="thema" size="sm" accessible-label="Thema op de kaart" class="thema" @change="thema = $event.target.value">
          <nldd-segmented-control-item v-for="t in THEMA" :key="t.id" :value="t.id" :text="t.kort" :selected="thema === t.id || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <nldd-text size="xs"><p><strong>{{ themaNu.tekst }}</strong></p></nldd-text>
        <!-- DSO-spoor: welke uitkomst de vakken van 40 m kleuren, met de aantallen. -->
        <template v-if="thema === 'dso'">
          <nldd-segmented-control :value="dsoLaag" size="sm" accessible-label="Wat de kaart kleurt" @change="dsoLaag = $event.target.value">
            <nldd-segmented-control-item v-for="l in dsoLagen" :key="l.id" :value="l.id" :text="l.kort" :selected="dsoLaag === l.id || undefined"></nldd-segmented-control-item>
          </nldd-segmented-control>
          <nldd-segmented-control :value="uitkomstWeergave" size="sm" accessible-label="Welk recht" @change="uitkomstWeergave = $event.target.value">
            <nldd-segmented-control-item value="geldend" text="Geldend recht" :selected="uitkomstWeergave === 'geldend' || undefined"></nldd-segmented-control-item>
            <nldd-segmented-control-item value="beoogd" text="Ontwerp-TAM" :selected="uitkomstWeergave === 'beoogd' || undefined"></nldd-segmented-control-item>
          </nldd-segmented-control>
          <nldd-text v-if="dsoFout" size="xs" color="critical"><p>Gegevens van het DSO-spoor niet geladen: {{ dsoFout }}</p></nldd-text>
          <template v-else>
            <nldd-text size="xs"><p><strong>{{ dsoLaagNu.tekst }}</strong></p></nldd-text>
            <template v-if="dsoLaag === 'sim'">
              <nldd-text size="xs" color="secondary"><p>{{ simTekst }}. <a href="#" @click.prevent="dsoDeel = 'simuleren'; openPaneel('dso')">Wijzigen</a></p></nldd-text>
              <nldd-progress-bar v-if="simVoortgang !== null" :value="Math.round(simVoortgang * 100)" max="100" size="sm" text="Simulatie rekenen"></nldd-progress-bar>
              <nldd-text v-if="simFout" size="xs" color="critical"><p>{{ simFout }}</p></nldd-text>
            </template>
            <div class="legenda-groep">
              <nldd-text size="xs" color="secondary"><p>{{ dsoLaagNu.uitleg }} Vakken van 40 m; vooraf gerekend voor 5 lagen van 3,2 m, open gegevens gunstig.</p></nldd-text>
              <span v-for="l in dsoLegenda" :key="l.klasse" class="legenda-item">
                <i class="lg-vlak" :style="{ borderColor: l.kleur, background: `color-mix(in srgb, ${l.kleur} 72%, transparent)` }"></i>
                <nldd-text size="xs"><span>{{ l.tekst }} ({{ dsoTelling[l.klasse] ?? 0 }})</span></nldd-text>
              </span>
              <span class="legenda-item"><i class="lg-nummer sc">{{ dsoReeks === 'besluiten' ? 'B1' : 'W1' }}</i><nldd-text size="xs"><span>Scenario ({{ dsoReeks === 'besluiten' ? 'besluiten' : 'woontoren' }}), <a href="#" @click.prevent="dsoDeel = 'scenarios'; openPaneel('dso')">lijst</a></span></nldd-text></span>
            </div>
          </template>
        </template>
        <template v-if="thema === 'uitkomst'">
          <nldd-segmented-control :value="uitkomstWeergave" size="sm" accessible-label="Welk recht" @change="uitkomstWeergave = $event.target.value">
            <nldd-segmented-control-item value="geldend" text="Geldend recht" :selected="uitkomstWeergave === 'geldend' || undefined"></nldd-segmented-control-item>
            <nldd-segmented-control-item value="beoogd" text="Ontwerp-TAM" :selected="uitkomstWeergave === 'beoogd' || undefined"></nldd-segmented-control-item>
          </nldd-segmented-control>
          <nldd-text size="xs" color="secondary">
            <p>
              {{ bouwplanTekst }}, {{ ingevuld ? `${ingevuld} aannames` : 'geen aannames' }}.
              <a href="#" @click.prevent="naarInvoer">Wijzigen</a>
            </p>
          </nldd-text>
          <nldd-progress-bar v-if="uitkomstVoortgang !== null" :value="Math.round(uitkomstVoortgang * 100)" max="100" size="sm" text="Uitkomst rekenen"></nldd-progress-bar>
          <nldd-text v-if="uitkomstFout" size="xs" color="critical"><p>{{ uitkomstFout }}</p></nldd-text>
        </template>
        <div v-if="themaNu.items.length" class="legenda-groep">
          <nldd-text size="xs" color="secondary"><p>{{ themaNu.uitleg }}</p></nldd-text>
          <span v-for="l in themaNu.items" :key="l.tekst" class="legenda-item">
            <i class="lg-vlak" :style="{ borderColor: l.kleur, background: `color-mix(in srgb, ${l.kleur} 72%, transparent)` }"></i>
            <nldd-text size="xs"><span>{{ l.tekst }}<template v-if="thema === 'uitkomst' && uitkomstTelling && uitkomstVoortgang === null"> ({{ uitkomstTelling[l.klasse] }})</template></span></nldd-text>
          </span>
          <a v-if="thema === 'uitkomst'" href="#" @click.prevent="naarUitleg('onthouding')"><nldd-text size="xs"><span>Wat is een onthouding?</span></nldd-text></a>
        </div>
        <div v-if="thema === 'plan'" class="legenda-groep">
          <nldd-text size="xs" color="secondary"><p>{{ themaNu.uitleg }}</p></nldd-text>
          <details class="overig">
            <summary><nldd-text size="xs"><span><strong>Alle {{ planLijst.length }} plannen</strong> (klik op een naam om erheen te gaan)</span></nldd-text></summary>
            <a v-for="pn in planLijst" :key="pn.id" href="#" class="plan-rij" @click.prevent="zoomNaarPlan(pn)">
              <i class="lg-plan" :style="{ background: pn.kleur, color: pn.licht ? '#1b1b1b' : '#fff' }">{{ pn.nr }}</i>
              <nldd-text size="xs"><span>{{ pn.naam }}<template v-if="pn.noot"> ({{ pn.noot }})</template></span></nldd-text>
            </a>
          </details>
        </div>
        <details v-if="thema === 'uitkomst' && ontbreektLijst.length && uitkomstVoortgang === null" class="overig ontbreekt" open>
          <summary><nldd-text size="xs"><span><strong>Wat ontbreekt nog</strong> (aantal vakken)</span></nldd-text></summary>
          <ul class="ontbreekt-lijst">
            <li v-for="o in ontbreektLijst" :key="o.id">
              <a v-if="o.invulbaar" href="#" @click.prevent="naarInvoer"><nldd-text size="xs"><span>{{ o.tekst }}</span></nldd-text></a>
              <nldd-text v-else size="xs" color="secondary"><span>{{ o.tekst }}</span></nldd-text>
              <nldd-text size="xs" color="secondary" class="aantal"><span>{{ o.aantal }}</span></nldd-text>
            </li>
          </ul>
        </details>
        <nldd-checkbox-field label="Bouwvlakken (waar een hoofdgebouw mag)" :checked="toonBouwvlak || undefined" @change="toonBouwvlak = $event.target.checked"></nldd-checkbox-field>
        <details class="overig">
        <summary><nldd-text size="xs"><span><strong>Overige tekens</strong></span></nldd-text></summary>
        <div v-for="g in LEGENDA" :key="g.kop" class="legenda-groep">
          <nldd-text size="xs" color="secondary"><p>{{ g.kop }}</p></nldd-text>
          <span v-for="l in g.items" :key="l.tekst" class="legenda-item">
            <i v-if="l.vorm === 'lijn'" class="lg-lijn" :style="{ borderTopColor: l.kleur, borderTopStyle: l.stippel ? 'dashed' : 'solid' }"></i>
            <i v-else-if="l.vorm === 'vlak'" class="lg-vlak" :style="{ borderColor: l.kleur, borderStyle: l.stippel ? 'dashed' : 'solid', background: `color-mix(in srgb, ${l.kleur} 14%, transparent)` }"></i>
            <i v-else-if="l.vorm === 'punt'" class="lg-punt"></i>
            <i v-else class="lg-nummer">P1</i>
            <nldd-text size="xs"><span>{{ l.tekst }}</span></nldd-text>
          </span>
        </div>
        </details>
      </div>

      <aside v-show="weergave === 'kaart'" ref="paneelEl" class="paneel" :class="{ breed: paneelBreed && !paneelKlein, klein: paneelKlein }" aria-label="Paneel bij de kaart">
        <div v-if="paneelKlein" class="paneel-klein">
          <div class="klein-rij">
          <nldd-button size="sm" appearance="primary" start-icon="chevron-double-left" text="Paneel openen" @click="openPaneel()"></nldd-button>
          <nldd-icon-button
            v-for="t in TABS" :key="t.id" size="sm" :icon="t.icoon" :text="t.tekst"
            :appearance="t.id === tab ? 'accent-filled' : 'neutral-transparent'"
            @click="openPaneel(t.id)"
          ></nldd-icon-button>
          </div>
          <nldd-text v-if="rd" size="xs" color="secondary" class="klein-punt"><p>{{ tabNu?.tekst }} · RD {{ Math.round(rd[0]) }}, {{ Math.round(rd[1]) }}</p></nldd-text>
        </div>
        <div v-show="!paneelKlein" class="paneel-kop">
          <nldd-tab-bar accessible-label="Paneel">
            <nldd-tab-bar-item text="Op dit punt" icon="map-pin" :current="tab === 'punt' || undefined" @click="kiesTab('punt')"></nldd-tab-bar-item>
            <nldd-tab-bar-item text="Toets" icon="checklist" :current="tab === 'toets' || undefined" @click="kiesTab('toets')"></nldd-tab-bar-item>
            <nldd-tab-bar-item text="Simuleren" icon="slider-horizontal-3" :current="tab === 'simuleren' || undefined" @click="kiesTab('simuleren')"></nldd-tab-bar-item>
            <nldd-tab-bar-item text="Testpunten" icon="list" :current="tab === 'testpunten' || undefined" @click="kiesTab('testpunten')"></nldd-tab-bar-item>
            <nldd-tab-bar-item text="Met en zonder DSO" icon="columns-2" :current="tab === 'dso' || undefined" @click="kiesTab('dso')"></nldd-tab-bar-item>
          </nldd-tab-bar>
          <!-- Uitleg is hulp, geen werktab: een eigen knop, altijd zichtbaar, ook als de tabs niet passen. -->
          <nldd-button
            v-if="paneelBreed" size="sm" start-icon="book" text="Uitleg"
            :appearance="tab === 'uitleg' ? 'accent-filled' : 'neutral-tinted'"
            @click="wisselUitleg"
          ></nldd-button>
          <nldd-icon-button v-else size="sm" icon="book" text="Uitleg: wat de termen betekenen" appearance="neutral-tinted" @click="wisselUitleg"></nldd-icon-button>
          <nldd-icon-button size="sm" icon="chevron-double-right" text="Paneel minimaliseren" appearance="neutral-transparent" @click="zetKlein(true)"></nldd-icon-button>
        </div>

        <div v-show="!paneelKlein" class="paneel-inhoud">
          <div v-show="tab === 'punt'">
            <nldd-banner v-if="fout" variant="critical" text="Kaartdata niet geladen" :supporting-text="fout"></nldd-banner>
            <FeitenPaneel v-else-if="stapel" :stapel="stapel" :rd="rd" lijst="simple" />
            <nldd-container v-else layout="stack" gap="12">
              <nldd-title size="display-3" :heading-level="2" text="Klik op een plek in de Spoorzone"></nldd-title>
              <nldd-text>
                <p>
                  Je ziet dan welke plannen daar liggen, welk plan ter plaatse de regels geeft, wat erbovenop komt en wat opzij gaat,
                  met de reden. Een vastgesteld TAM-plan schakelt alles eronder uit. Anders geldt het nieuwste basisplan, met de
                  paraplu alleen volgens zijn tabellen.
                </p>
              </nldd-text>
              <nldd-text size="sm" color="secondary"><p>Of kies een plek onder Testpunten. Verkenning, geen besluit.</p></nldd-text>
            </nldd-container>
          </div>

          <ToetsTab v-if="tab === 'toets'" :stapel="stapel" :rd="rd" />
          <SimulerenTab v-if="tab === 'simuleren'" v-model:deel="simDeel" :stapel="stapel" :rd="rd" @uitleg="naarUitleg" @kaart="toonUitkomstOpKaart" />
          <UitlegTab v-if="tab === 'uitleg'" @sluit="sluitUitleg" />
          <DsoTab
            v-if="tab === 'dso'" v-model:deel="dsoDeel" v-model:reeks="dsoReeks"
            :data="dso" :fout="dsoFout" :cel="dsoCel" :index="dsoCelIndex" :scenario="dsoScenario" :weergave="uitkomstWeergave"
            @artikel="artikel = $event" @scenario="kiesScenario" @kaart="toonSimOpKaart"
          />

          <nldd-container v-show="tab === 'testpunten'" layout="stack" gap="12">
            <nldd-title size="display-5" :heading-level="2" text="Testpunten en voorbeeldplekken"
              supporting-text="Kies een plek; de kaart springt erheen en je ziet wat daar geldt."></nldd-title>
            <nldd-list appearance="simple" accessible-label="Testpunten">
              <nldd-list-item
                v-for="t in testpunten"
                :key="t.naam"
                button
                :selected="t.naam === gekozenTestpunt || undefined"
                @click="kiesTestpunt(t)"
              >
                <nldd-title-cell size="display-6" :text="t.naam" :supporting-text="t.waarom"></nldd-title-cell>
                <nldd-cell width="fit-content">
                  <nldd-tag :color="t.soort === 'testpunt' ? 'accent' : 'neutral'" :text="t.soort === 'testpunt' ? 'testpunt' : 'voorbeeld'"></nldd-tag>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
            <nldd-text size="sm" color="secondary">
              <p>
                P1 tot en met P4 zijn de testpunten voor de toets en staan genummerd op de kaart.
                Stationsplein en Industrielaan liggen op bestemming Verkeer.
              </p>
            </nldd-text>
          </nldd-container>
        </div>
      </aside>
      <ArtikelSheet v-model:sleutel="artikel" :data="dso" />
    </div>
  </div>
</template>

<style scoped>
.kaarttab { height: 100%; box-sizing: border-box; padding: 16px; display: flex; }
.kaartkader {
  position: relative;
  display: flex;
  flex: 1;
  min-height: 420px;
  border: 1px solid var(--primitives-color-neutral-300, #c8c8c8);
  border-radius: 8px;
  overflow: hidden;
}
/* De schakelaar Kaart | Graaf linksboven, naast de zoomknoppen; de graaf over de hele ruimte. */
.weergave-wissel { position: absolute; top: 10px; left: 56px; z-index: 1200; }
.graaflaag { z-index: 1100; }

/* Leaflet zet zelf position: relative op de kaart; de kaart vult het kader via flex. */
.kaart { flex: 1; min-width: 0; }

/* Achtergrondkaart rustiger: grijzer en iets lichter. Alleen de tegels, niet de plangrenzen. */
.kaart :deep(.leaflet-tile-pane) { filter: saturate(0) contrast(0.86) brightness(1.06); }
@media (prefers-color-scheme: dark) {
  .kaart :deep(.leaflet-tile-pane) { filter: grayscale(0.8) brightness(0.72) contrast(1.05); }
}

/* Gedeelde stijl van wat over de kaart ligt: half doorzichtig, met een lichte vervaging. */
.legenda,
.paneel {
  background: color-mix(in srgb, var(--semantics-surfaces-base-background-color, Canvas) 74%, transparent);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
}

/* Legenda linksonder, boven de bronvermelding. */
.legenda {
  position: absolute;
  left: 12px;
  bottom: 30px;
  z-index: 650;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 12px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--semantics-surfaces-base-background-color, Canvas) 90%, transparent);
  border: 1px solid color-mix(in srgb, var(--primitives-color-neutral-400, #999) 35%, transparent);
  box-shadow: 0 2px 10px rgb(0 0 0 / 0.10);
  max-width: 460px; /* DSO-spoor: breder, voor het zesde thema en de lagen van het DSO-spoor */
  max-height: calc(100% - 120px);
  overflow-y: auto;
  gap: 2px;
}
.thema { margin: 4px 0 2px; }
.overig summary { cursor: pointer; margin-top: 6px; }
.overig summary nldd-text { display: inline-block; }
.overig[open] summary { margin-bottom: 2px; }
.ontbreekt-lijst { list-style: none; margin: 2px 0 0; padding: 0; display: flex; flex-direction: column; gap: 1px; }
.ontbreekt-lijst li { display: flex; justify-content: space-between; gap: 10px; }
.ontbreekt-lijst .aantal { flex: none; }
.legenda-groep { display: flex; flex-direction: column; gap: 1px; margin-top: 3px; }
.legenda-item { display: inline-flex; align-items: center; gap: 8px; }
.legenda-item i { display: inline-block; flex: none; }
.lg-lijn { width: 20px; height: 0; border-top-width: 2px; }
.lg-vlak { width: 18px; height: 12px; border-width: 1.5px; border-radius: 2px; }
.lg-punt { width: 12px; height: 12px; border-radius: 50%; background: #ffb612; border: 2px solid #000; box-sizing: border-box; }
.lg-nummer { font-style: normal; font-size: 0.65rem; font-weight: 700; padding: 1px 4px; border-radius: 4px; background: #154273; color: #fff; }
/* DSO-spoor: scenario's in violet, los van de testpunten van de eerste PoC (donkerblauw). */
.lg-nummer.sc, :deep(.sc-icoon span) { background: #42145f; }
.plan-rij { display: flex; align-items: baseline; gap: 8px; text-decoration: none; color: inherit; padding: 1px 0; }
.plan-rij:hover nldd-text, .plan-rij:focus-visible nldd-text { text-decoration: underline; }
.lg-plan, :deep(.plan-nr span) {
  font: 700 11px/1 var(--primitives-font-family-body, system-ui, sans-serif);
  font-style: normal; padding: 3px 0; border-radius: 4px; text-align: center;
}
.lg-plan { flex: none; width: 22px; }
:deep(.plan-nr span) {
  position: absolute; transform: translate(-50%, -50%); white-space: nowrap;
  min-width: 22px; padding: 3px 5px; box-sizing: border-box; border: 1.5px solid #fff; box-shadow: 0 1px 4px rgb(0 0 0 / 0.35);
}
:deep(.tp-icoon) { display: flex; align-items: center; justify-content: center; }
:deep(.tp-icoon span) {
  font: 700 11px/1 var(--primitives-font-family-body, system-ui, sans-serif);
  padding: 4px 6px; border-radius: 5px; background: #154273; color: #fff;
  box-shadow: 0 1px 4px rgb(0 0 0 / 0.35); border: 1.5px solid #fff; cursor: pointer;
}

/* Het paneel: vast rechts over de kaart, eigen scroll. Boven de kaart en de bediening. */
.paneel {
  position: absolute;
  top: 12px;
  right: 12px;
  bottom: 12px;
  z-index: 1000;
  width: clamp(380px, 40vw, 620px);
  transition: width 0.18s ease;
  display: flex;
  flex-direction: column;
  border-radius: 10px;
  border: 1px solid color-mix(in srgb, var(--primitives-color-neutral-400, #999) 40%, transparent);
  box-shadow: 0 6px 24px rgb(0 0 0 / 0.14);
  overflow: hidden;
}
.paneel-kop { padding: 12px 16px 8px; display: flex; align-items: center; gap: 8px; }
.paneel-kop nldd-tab-bar { flex: 1; min-width: 0; }
.paneel.breed { width: clamp(460px, 54vw, 960px); }
/* Geminimaliseerd: een kleine kaart rechtsboven, de kaart is weer vrij. */
.paneel.klein { bottom: auto; width: max-content; max-width: calc(100% - 24px); }
.paneel-klein { display: flex; flex-direction: column; gap: 4px; padding: 8px 10px; }
.klein-rij { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; }
.klein-punt { padding: 0 4px; }
.paneel-inhoud {
  flex: 1; min-height: 0; overflow-y: auto; padding: 8px 16px 20px; overscroll-behavior: contain;
  container-type: inline-size; container-name: paneel;
}

@media (max-width: 800px) {
  /* Smal scherm: het paneel onder de kaart in plaats van erover. */
  .kaartkader { flex-direction: column; }
  .paneel { position: static; width: auto; max-height: 55vh; border-radius: 0; box-shadow: none; }
}
</style>
