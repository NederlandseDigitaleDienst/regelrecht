<script setup>
// De kaart met alles erop en eraan: plangrenzen, kaartthema's met legenda linksonder, testpunten,
// het gekozen punt, en het paneel rechts met de tabs. De inhoud van de tabs komt uit losse
// componenten; zet ze in het blok "paneel-inhoud" hieronder. Vul alles met {{...}} per casus.
import 'leaflet/dist/leaflet.css';
import L from 'leaflet';
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import ArtikelSheet from './ArtikelSheet.vue';
import { ACCENT, BESTEMMING, HOOGTE, KLASSEN, KLEUR_PLAN, PUNT, ZOEKGEBIED } from '../kleuren.js';
import { naarRd, rdTekst, vanRd } from '../geo.js';

const kaartEl = ref(null);
const paneelEl = ref(null);
const fout = ref(null);
const rd = ref(null); // het gekozen punt in RD, of null
const testpunten = ref([]);
const gekozenTestpunt = ref(null);
const artikel = ref(null); // open artikel in de sheet rechts
const urlParams = new URLSearchParams(window.location.search);
let kaart = null;
let marker = null;
let canvas = null;

// --- Tabs. Vaste volgorde en iconen; laat weg wat de casus niet heeft. Uitleg is hulp, geen werktab.
const TABS = [
  { id: 'punt', tekst: 'Op dit punt', icoon: 'map-pin' },
  { id: 'toets', tekst: 'Toets', icoon: 'checklist' },
  { id: 'simuleren', tekst: 'Simuleren', icoon: 'slider-horizontal-3' },
  { id: 'testpunten', tekst: 'Testpunten', icoon: 'list' },
];
const UITLEG = { id: 'uitleg', tekst: 'Uitleg', icoon: 'book' };
// Tabs met veel inhoud krijgen een breder paneel.
const BREED = new Set(['toets', 'simuleren', 'uitleg']);
const tab = ref([...TABS, UITLEG].some((t) => t.id === urlParams.get('tab')) ? urlParams.get('tab') : 'punt');
const paneelBreed = computed(() => BREED.has(tab.value));
const tabNu = computed(() => [...TABS, UITLEG].find((t) => t.id === tab.value));
const vorigeTab = ref('punt');

function kiesTab(t) {
  if (t === 'uitleg' && tab.value !== 'uitleg') vorigeTab.value = tab.value;
  tab.value = t;
  zetUrl('tab', t, 'punt');
  // Het paneel verandert van breedte; laat Leaflet de nieuwe maat zien.
  requestAnimationFrame(() => kaart?.invalidateSize());
}
function wisselUitleg() { kiesTab(tab.value === 'uitleg' ? vorigeTab.value : 'uitleg'); }
function opToets(e) { if (e.key === 'Escape' && tab.value === 'uitleg') kiesTab(vorigeTab.value); }

// Paneel minimaliseren: klein rechtsboven, per tab een knop. Onthouden per browser.
const OPSLAG = '%%SLUG%%-paneel-klein';
const leesKlein = () => { try { return localStorage.getItem(OPSLAG) === '1'; } catch { return false; } };
const paneelKlein = ref(leesKlein());
function zetKlein(k) {
  paneelKlein.value = k;
  try { localStorage.setItem(OPSLAG, k ? '1' : '0'); } catch { /* geen opslag: alleen deze sessie */ }
  requestAnimationFrame(() => kaart?.invalidateSize());
}
function openPaneel(t) { zetKlein(false); if (t && t !== tab.value) kiesTab(t); }
const paneelBreedte = () => paneelEl.value?.getBoundingClientRect().width ?? 0;

// --- Kaart of graaf in de grote ruimte. De kaart blijft eronder staan.
const weergave = ref(urlParams.get('weergave') === 'graaf' ? 'graaf' : 'kaart');
function kiesWeergave(w) {
  weergave.value = w;
  zetUrl('weergave', w, 'kaart');
  if (w === 'kaart') requestAnimationFrame(() => kaart?.invalidateSize());
}

// --- Kaartthema's: één tegelijk. items = de legenda; uitleg = één zin onder de titel.
const THEMA = [
  { id: 'plan', kort: 'Plan', tekst: 'Welk plan ligt waar', uitleg: 'Het plan dat op een plek de regels geeft. Het nieuwste plan ligt bovenop.', items: [] },
  { id: 'hoogte', kort: 'Hoogte', tekst: 'Maximale bouwhoogte', uitleg: 'Maatvoering op de plankaart van het plan dat hier geldt.', items: HOOGTE },
  { id: 'bestemming', kort: 'Functie', tekst: 'Bestemming', uitleg: 'Hoofdgroep uit de plankaart van het plan dat hier geldt.', items: BESTEMMING },
  { id: 'uitkomst', kort: 'Uitkomst', tekst: 'Uitkomst van de toets', uitleg: 'Met het bouwplan en de aannames uit Simuleren.', items: KLASSEN },
];
const thema = ref(THEMA.some((t) => t.id === urlParams.get('thema')) ? urlParams.get('thema') : '%%STANDAARD_THEMA%%');
const themaNu = computed(() => THEMA.find((t) => t.id === thema.value) ?? THEMA[0]);
const telling = ref(null); // { klasse: aantal } voor het huidige thema, als die er is
const toonBouwvlak = ref(true);
function kiesThema(t) {
  thema.value = t;
  zetUrl('thema', t, '%%STANDAARD_THEMA%%');
  // %%THEMA_TEKENEN%%: bouw de laag van dit thema (canvas-renderer, pane 'context') en zet hem aan.
}

// Overige tekens, in groepen. Vorm: lijn, vlak, punt, nummer.
const LEGENDA = [
  { kop: 'Gebied', items: [{ tekst: 'Zoekgebied (afgeleid, geen officiële grens)', vorm: 'lijn', kleur: ZOEKGEBIED, stippel: true }] },
  { kop: 'Plannen', items: [
    { tekst: 'Bestemmingsplan', vorm: 'vlak', kleur: KLEUR_PLAN.basisplan },
    { tekst: 'Omgevingsplan, nieuw deel', vorm: 'vlak', kleur: KLEUR_PLAN.tam },
    { tekst: 'Ontwerp', vorm: 'vlak', kleur: KLEUR_PLAN['tam-ontwerp'], stippel: true },
  ] },
  { kop: 'Op het gekozen punt', items: [
    { tekst: 'Gekozen punt', vorm: 'punt' },
    { tekst: 'Bestemming of locatie', vorm: 'vlak', kleur: ACCENT },
    { tekst: 'Bouwvlak', vorm: 'vlak', kleur: '#000', stippel: true },
    { tekst: 'Testpunt', vorm: 'nummer' },
  ] },
];

// --- Deellinks: standaardwaarden blijven uit de URL.
function zetUrl(sleutel, waarde, standaard) {
  const url = new URL(window.location.href);
  if (waarde && waarde !== standaard) url.searchParams.set(sleutel, waarde); else url.searchParams.delete(sleutel);
  window.history.replaceState(null, '', url);
}

// --- Een punt kiezen.
function kies(lonlat) {
  rd.value = naarRd(lonlat);
  // In Toets of Simuleren blijven: die rekenen meteen op het nieuwe punt.
  if (!BREED.has(tab.value)) tab.value = 'punt';
  zetUrl('rd', `${Math.round(rd.value[0])},${Math.round(rd.value[1])}`);
  const ll = [lonlat[1], lonlat[0]];
  if (marker) marker.setLatLng(ll);
  else marker = L.circleMarker(ll, { radius: 7, color: '#000', weight: 2, fillColor: PUNT, fillOpacity: 1 }).addTo(kaart);
  // %%ACCENT_OP_PUNT%%: de bestemming (oranje, weight 2) en het bouwvlak (zwart, gestippeld 4 3) op dit punt.
}
function kiesRd(xy, zoom = 17) {
  const lonlat = vanRd(xy);
  kaart.setView([lonlat[1], lonlat[0]], zoom, { animate: false });
  // Het punt in het midden van het vrije deel, links van het paneel.
  kaart.panBy([paneelBreedte() / 2, 0], { animate: false });
  kies(lonlat);
}
function kiesTestpunt(t) { gekozenTestpunt.value = t.naam; kiesRd(t.rd); }

onMounted(async () => {
  window.addEventListener('keydown', opToets);
  kaart = L.map(kaartEl.value, { zoomControl: true, attributionControl: false }).setView([%%START_LAT%%, %%START_LON%%], 15);
  // Bronvermelding linksonder: rechts ligt het paneel.
  L.control.attribution({ position: 'bottomleft', prefix: false }).addTo(kaart);
  L.tileLayer('https://service.pdok.nl/brt/achtergrondkaart/wmts/v2_0/grijs/EPSG:3857/{z}/{x}/{y}.png', {
    maxZoom: 19, attribution: 'Kaart: <a href="https://www.pdok.nl">PDOK</a>, BRT Achtergrondkaart grijs (Kadaster)',
  }).addTo(kaart);
  new ResizeObserver(() => kaart?.invalidateSize()).observe(kaartEl.value);
  // Contextthema's onder de plangrenzen, op canvas: duizenden vlakken.
  kaart.createPane('context').style.zIndex = 350;
  canvas = L.canvas({ pane: 'context', padding: 0.2 });

  try {
    // %%KAARTDATA_LADEN%%: zoekgebied, plangrenzen, contextlagen.
    const r = await fetch(`${import.meta.env.BASE_URL}data/testpunten.json`);
    testpunten.value = r.ok ? (await r.json()).punten ?? [] : [];
  } catch (e) {
    fout.value = String(e?.message ?? e);
  }
  // Eerst de layout afwachten; anders past fitBounds in een kaart van nul pixels.
  await new Promise((klaar) => requestAnimationFrame(() => klaar()));
  kaart.invalidateSize();

  // Zoekgebied, als het er is:
  // const zoek = L.geoJSON(zoekgebied, { style: { color: ZOEKGEBIED, weight: 2, dashArray: '6 4', fill: false }, interactive: false }).addTo(kaart);
  // kaart.fitBounds(zoek.getBounds(), { paddingBottomRight: [paneelBreedte() + 24, 0], paddingTopLeft: [24, 24], animate: false });
  // Plangrenzen per plan: color KLEUR_PLAN[rol], weight 1.5 (groot onderliggend plan 0.5),
  // fillOpacity 0.07 (groot 0.03), dashArray '5 4' voor een ontwerp, interactive: false.

  // Testpunten als genummerde labels.
  for (const t of testpunten.value.filter((x) => x.toets)) {
    const ll = vanRd(t.rd);
    L.marker([ll[1], ll[0]], {
      icon: L.divIcon({ className: 'tp-icoon', html: `<span>${t.naam.split(' ')[0]}</span>`, iconSize: [28, 20], iconAnchor: [14, 10] }),
      title: t.naam, keyboard: true,
    }).on('click', (e) => { L.DomEvent.stopPropagation(e); kiesTestpunt(t); }).addTo(kaart);
  }
  kaart.on('click', (e) => { gekozenTestpunt.value = null; kies([e.latlng.lng, e.latlng.lat]); });
  kiesThema(thema.value);
  // Deellink: ?rd=x,y opent direct op dat punt.
  const xy = urlParams.get('rd')?.split(',').map(Number);
  if (xy?.length === 2 && xy.every(Number.isFinite)) kiesRd(xy);
});
onBeforeUnmount(() => { window.removeEventListener('keydown', opToets); kaart?.remove(); });
</script>

<template>
  <div class="kaarttab">
    <div class="kaartkader">
      <div ref="kaartEl" class="kaart" role="application" aria-label="Kaart van %%GEBIED%%. Klik op een plek om te zien welke regels daar gelden."></div>

      <nldd-segmented-control class="weergave-wissel" size="sm" :value="weergave" accessible-label="Weergave" @change="kiesWeergave($event.target.value)">
        <nldd-segmented-control-item value="kaart" text="Kaart" icon="map" :selected="weergave === 'kaart' || undefined"></nldd-segmented-control-item>
        <nldd-segmented-control-item value="graaf" text="Graaf" icon="centralized-network" :selected="weergave === 'graaf' || undefined"></nldd-segmented-control-item>
      </nldd-segmented-control>
      <!-- %%GRAAF%%: <GraafTab v-if="weergave === 'graaf'" :rd="rd" class="graaflaag" /> (vue-flow, over de hele kaart) -->

      <div v-show="weergave === 'kaart'" class="legenda" aria-label="Kaartlagen en legenda">
        <nldd-text size="sm"><p><strong>Kaartlagen</strong></p></nldd-text>
        <nldd-segmented-control :value="thema" size="sm" accessible-label="Thema op de kaart" class="thema" @change="kiesThema($event.target.value)">
          <nldd-segmented-control-item v-for="t in THEMA" :key="t.id" :value="t.id" :text="t.kort" :selected="thema === t.id || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <nldd-text size="xs"><p><strong>{{ themaNu.tekst }}</strong></p></nldd-text>
        <div class="legenda-groep">
          <nldd-text size="xs" color="secondary"><p>{{ themaNu.uitleg }}</p></nldd-text>
          <span v-for="l in themaNu.items" :key="l.tekst" class="legenda-item">
            <i class="lg-vlak" :style="{ borderColor: l.kleur, background: `color-mix(in srgb, ${l.kleur} 72%, transparent)` }"></i>
            <nldd-text size="xs"><span>{{ l.tekst }}<template v-if="telling && l.klasse"> ({{ telling[l.klasse] ?? 0 }})</template></span></nldd-text>
          </span>
        </div>
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
              v-for="t in [...TABS, UITLEG]" :key="t.id" size="sm" :icon="t.icoon" :text="t.tekst"
              :appearance="t.id === tab ? 'accent-filled' : 'neutral-transparent'" @click="openPaneel(t.id)"
            ></nldd-icon-button>
          </div>
          <nldd-text v-if="rd" size="xs" color="secondary" class="klein-punt"><p>{{ tabNu?.tekst }} · RD {{ rdTekst(rd) }}</p></nldd-text>
        </div>
        <div v-show="!paneelKlein" class="paneel-kop">
          <nldd-tab-bar accessible-label="Paneel">
            <nldd-tab-bar-item v-for="t in TABS" :key="t.id" :text="t.tekst" :icon="t.icoon" :current="tab === t.id || undefined" @click="kiesTab(t.id)"></nldd-tab-bar-item>
          </nldd-tab-bar>
          <nldd-button v-if="paneelBreed" size="sm" start-icon="book" text="Uitleg" :appearance="tab === 'uitleg' ? 'accent-filled' : 'neutral-tinted'" @click="wisselUitleg"></nldd-button>
          <nldd-icon-button v-else size="sm" icon="book" text="Uitleg: wat de termen betekenen" appearance="neutral-tinted" @click="wisselUitleg"></nldd-icon-button>
          <nldd-icon-button size="sm" icon="chevron-double-right" text="Paneel minimaliseren" appearance="neutral-transparent" @click="zetKlein(true)"></nldd-icon-button>
        </div>

        <div v-show="!paneelKlein" class="paneel-inhoud">
          <div v-show="tab === 'punt'">
            <nldd-banner v-if="fout" variant="critical" text="Kaartdata niet geladen" :supporting-text="fout"></nldd-banner>
            <!-- %%OP_DIT_PUNT%%: <FeitenPaneel v-else-if="rd" :rd="rd" /> — plannen, rangorde, met de reden. -->
            <nldd-container v-else-if="rd" layout="stack" gap="12">
              <nldd-title size="display-5" :heading-level="2" :text="`Plek bij RD ${rdTekst(rd)}`" overline="Geldend recht"></nldd-title>
            </nldd-container>
            <nldd-container v-else layout="stack" gap="12">
              <nldd-title size="display-3" :heading-level="2" text="Klik op een plek in %%GEBIED%%"></nldd-title>
              <nldd-text><p>%%LEGE_STAAT_UITLEG%%</p></nldd-text>
              <nldd-text size="sm" color="secondary"><p>Of kies een plek onder Testpunten. Verkenning, geen besluit.</p></nldd-text>
            </nldd-container>
          </div>

          <!-- %%TOETS%%: <ToetsTab v-if="tab === 'toets'" :rd="rd" @artikel="artikel = $event" /> (gebruik UitkomstBlok.vue) -->
          <!-- %%SIMULEREN%%: <SimulerenTab v-if="tab === 'simuleren'" :rd="rd" /> -->
          <!-- %%UITLEG%%: <UitlegTab v-if="tab === 'uitleg'" @sluit="wisselUitleg" /> (ankers uitleg-<begrip>) -->

          <nldd-container v-show="tab === 'testpunten'" layout="stack" gap="12">
            <nldd-title size="display-5" :heading-level="2" text="Testpunten en voorbeeldplekken" supporting-text="Kies een plek; de kaart springt erheen en je ziet wat daar geldt."></nldd-title>
            <nldd-list appearance="simple" accessible-label="Testpunten">
              <nldd-list-item v-for="t in testpunten" :key="t.naam" button :selected="t.naam === gekozenTestpunt || undefined" @click="kiesTestpunt(t)">
                <nldd-title-cell size="display-6" :text="t.naam" :supporting-text="t.waarom"></nldd-title-cell>
                <nldd-cell width="fit-content">
                  <nldd-tag :color="t.soort === 'testpunt' ? 'accent' : 'neutral'" :text="t.soort === 'testpunt' ? 'testpunt' : 'voorbeeld'"></nldd-tag>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
        </div>
      </aside>
      <ArtikelSheet v-model:artikel="artikel" />
    </div>
  </div>
</template>

<style scoped>
.kaarttab { height: 100%; box-sizing: border-box; padding: 16px; display: flex; }
.kaartkader {
  position: relative; display: flex; flex: 1; min-height: 420px;
  border: 1px solid var(--primitives-color-neutral-300, #c8c8c8); border-radius: 8px; overflow: hidden;
}
/* Kaart | Graaf linksboven, naast de zoomknoppen; de graaf over de hele ruimte. */
.weergave-wissel { position: absolute; top: 10px; left: 56px; z-index: 1200; }
.graaflaag { z-index: 1100; }
.kaart { flex: 1; min-width: 0; }

/* Achtergrondkaart rustiger. Alleen de tegels, niet de plangrenzen. */
.kaart :deep(.leaflet-tile-pane) { filter: saturate(0) contrast(0.86) brightness(1.06); }
@media (prefers-color-scheme: dark) {
  .kaart :deep(.leaflet-tile-pane) { filter: grayscale(0.8) brightness(0.72) contrast(1.05); }
}

/* Wat over de kaart ligt: half doorzichtig, met een lichte vervaging. */
.legenda, .paneel {
  background: color-mix(in srgb, var(--semantics-surfaces-base-background-color, Canvas) 74%, transparent);
  backdrop-filter: blur(6px); -webkit-backdrop-filter: blur(6px);
}

/* Legenda linksonder, boven de bronvermelding. */
.legenda {
  position: absolute; left: 12px; bottom: 30px; z-index: 650;
  display: flex; flex-direction: column; gap: 2px; padding: 8px 12px; border-radius: 8px;
  background: color-mix(in srgb, var(--semantics-surfaces-base-background-color, Canvas) 90%, transparent);
  border: 1px solid color-mix(in srgb, var(--primitives-color-neutral-400, #999) 35%, transparent);
  box-shadow: 0 2px 10px rgb(0 0 0 / 0.10);
  max-width: 460px; max-height: calc(100% - 120px); overflow-y: auto;
}
.thema { margin: 4px 0 2px; }
.overig summary { cursor: pointer; margin-top: 6px; }
.overig summary nldd-text { display: inline-block; }
.overig[open] summary { margin-bottom: 2px; }
.legenda-groep { display: flex; flex-direction: column; gap: 1px; margin-top: 3px; }
.legenda-item { display: inline-flex; align-items: center; gap: 8px; }
.legenda-item i { display: inline-block; flex: none; }
.lg-lijn { width: 20px; height: 0; border-top-width: 2px; }
.lg-vlak { width: 18px; height: 12px; border-width: 1.5px; border-radius: 2px; border-style: solid; }
.lg-punt { width: 12px; height: 12px; border-radius: 50%; background: #ffb612; border: 2px solid #000; box-sizing: border-box; }
.lg-nummer { font-style: normal; font-size: 0.65rem; font-weight: 700; padding: 1px 4px; border-radius: 4px; background: #154273; color: #fff; }
:deep(.tp-icoon) { display: flex; align-items: center; justify-content: center; }
:deep(.tp-icoon span) {
  font: 700 11px/1 var(--primitives-font-family-body, system-ui, sans-serif);
  padding: 4px 6px; border-radius: 5px; background: #154273; color: #fff;
  box-shadow: 0 1px 4px rgb(0 0 0 / 0.35); border: 1.5px solid #fff; cursor: pointer;
}

/* Het paneel: vast rechts over de kaart, eigen scroll. */
.paneel {
  position: absolute; top: 12px; right: 12px; bottom: 12px; z-index: 1000;
  width: clamp(380px, 40vw, 620px); transition: width 0.18s ease;
  display: flex; flex-direction: column; border-radius: 10px; overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--primitives-color-neutral-400, #999) 40%, transparent);
  box-shadow: 0 6px 24px rgb(0 0 0 / 0.14);
}
.paneel.breed { width: clamp(460px, 54vw, 960px); }
.paneel.klein { bottom: auto; width: max-content; max-width: calc(100% - 24px); }
.paneel-kop { padding: 12px 16px 8px; display: flex; align-items: center; gap: 8px; }
.paneel-kop nldd-tab-bar { flex: 1; min-width: 0; }
.paneel-klein { display: flex; flex-direction: column; gap: 4px; padding: 8px 10px; }
.klein-rij { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; }
.klein-punt { padding: 0 4px; }
.paneel-inhoud {
  flex: 1; min-height: 0; overflow-y: auto; padding: 8px 16px 20px; overscroll-behavior: contain;
  container-type: inline-size; container-name: paneel;
}

@media (max-width: 800px) {
  /* Smal scherm: kaart, legenda en paneel onder elkaar; de pagina scrollt. */
  .kaarttab { height: auto; min-height: 100%; padding: 8px; }
  .kaartkader { flex-direction: column; min-height: 0; }
  .kaart { flex: none; height: 60vh; }
  .legenda, .paneel { position: static; width: auto; max-width: none; max-height: none; border-radius: 0; box-shadow: none; border-width: 1px 0 0; }
}
</style>
