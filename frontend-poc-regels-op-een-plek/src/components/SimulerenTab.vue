<script setup>
// Simuleren, naar de Simuleren-tab van de eerste PoC: het bouwplan en de aannames aanpassen, de
// uitkomst op dit vak zien, per aantal lagen zien waar een regel omslaat, en de kaart opnieuw kleuren.
// Rekent het DSO-spoor opnieuw in de browser (sim.js); de eerste PoC blijft zoals vooraf gerekend.
import { computed, onMounted, ref, shallowRef, watch } from 'vue';
import Uitklap from './Uitklap.vue';
import { BANNER, OVERHEID, TAG, TEKST, UIT_KLASSE, kortId } from '../data.js';
import { KEUZE, WONINGTYPES, aannames, bouwplan, hoogte, isStandaard, lagenScan, laadMotor, rekenVak, standaard } from '../sim.js';

const props = defineProps({ cel: Object, index: Number, weergave: String, data: Object });
const emit = defineEmits(['kaart']);

const motor = shallowRef(null);
const fout = ref(null);
const bezig = ref(false);
const res = shallowRef(null);
const scan = shallowRef(null);
const gegevensOpen = ref(false);
const lagenOpen = ref(true);
const toon = ref('omslaan');

onMounted(async () => {
  try { motor.value = await laadMotor(); reken(); } catch (e) { fout.value = String(e?.message ?? e); }
});

let wacht = null;
function reken() {
  if (!motor.value || props.index == null || props.index < 0) { res.value = null; scan.value = null; return; }
  bezig.value = true;
  clearTimeout(wacht);
  wacht = setTimeout(() => {
    try {
      res.value = rekenVak(motor.value, props.index, props.cel.rd);
      scan.value = lagenScan(motor.value, props.index, props.cel.rd);
      fout.value = null;
    } catch (e) { fout.value = String(e?.message ?? e); }
    bezig.value = false;
  }, 120);
}
watch([() => ({ ...bouwplan }), () => ({ ...aannames }), () => props.index, () => props.weergave], reken, { deep: true });

const beoogd = computed(() => props.weergave === 'beoogd');
function kolom(r) { return beoogd.value && r.beoogd ? r.nieuw : (r.tijdelijk.status === 'niet-van-toepassing' ? r.nieuw : r.tijdelijk); }
const klasseVan = (r) => (beoogd.value ? r.beoogd : r.geldend);
const standaardKlasse = computed(() => (beoogd.value ? props.cel.dso.beoogd : props.cel.dso.geldend));
const totaal = computed(() => (res.value?.totaal ? UIT_KLASSE[res.value.totaal] : 'open'));
const banner = (k) => BANNER[k] ?? { variant: 'neutral', tekst: 'Geen uitkomst' };
const kol = computed(() => (res.value ? kolom(res.value) : null));

// --- Tabel per aantal lagen.
const STAAT = {
  toegestaan: { klasse: 'goed', kort: 'ja', tekst: 'toegestaan' },
  onthouding: { klasse: 'open', kort: '?', tekst: 'onthouding' },
  'niet toegestaan': { klasse: 'kritiek', kort: 'nee', tekst: 'niet toegestaan' },
};
const staat = (u) => STAAT[u] ?? { klasse: '', kort: '–', tekst: u ?? 'niet gerekend' };
const label = (k) => { const a = motor.value?.sim.artikelen[k]; return a ? `${kortId(a.regeling)}, art. ${a.artikel}` : k; };
const LEESBAAR = { BOUW: 'Bouwregels', GEBRUIK: 'Gebruik (bestemming)', HUB: 'Eindoordeel gemeente (22.29)' };
const tabel = computed(() => {
  if (!scan.value) return null;
  const kolommen = scan.value.runs.map(kolom);
  if (kolommen.every((k) => k.status !== 'gerekend')) return { status: kolommen[0].status, reden: kolommen[0].reden };
  const rijen = new Map();
  kolommen.forEach((k, i) => {
    for (const s of k.stappen ?? []) {
      if (!['BOUW', 'GEBRUIK', 'HUB'].includes(s.vorm)) continue;
      const sleutel = `${s.vorm}|${s.sleutel}`;
      if (!rijen.has(sleutel)) rijen.set(sleutel, { vorm: s.vorm, sleutel: s.sleutel, uitkomsten: Array(kolommen.length).fill(null) });
      rijen.get(sleutel).uitkomsten[i] = s.uitkomst;
    }
  });
  const alle = [...rijen.values()].sort((a, b) => (a.vorm === 'HUB' ? -1 : b.vorm === 'HUB' ? 1 : 0));
  if (!beoogd.value) alle.push({ vorm: 'TOTAAL', sleutel: 'totaal', uitkomsten: scan.value.runs.map((r) => r.totaal) });
  const omslaan = (r) => r.uitkomsten.some((u, i) => i > 0 && u !== r.uitkomsten[i - 1]);
  const getoond = toon.value === 'alle' ? alle : alle.filter((r) => r.vorm === 'HUB' || omslaan(r));
  const kantel = [];
  for (const r of alle) {
    r.uitkomsten.forEach((u, i) => {
      if (i > 0 && u !== r.uitkomsten[i - 1]) {
        kantel.push({ eind: r.vorm === 'HUB' || r.vorm === 'TOTAAL', regel: r.vorm === 'TOTAAL' ? 'Alle overheden samen' : `${label(r.sleutel)} · ${LEESBAAR[r.vorm]}`,
          tekst: `Van ${scan.value.lagen[i - 1]} naar ${scan.value.lagen[i]} lagen (${meter(scan.value.hoogtes[i - 1])} → ${meter(scan.value.hoogtes[i])}): ${staat(r.uitkomsten[i - 1]).tekst} → ${staat(u).tekst}` });
      }
    });
  }
  return { status: 'gerekend', rijen: getoond, verborgen: alle.length - getoond.length, kantel };
});
const isKantel = (r, i) => i > 0 && r.uitkomsten[i] !== r.uitkomsten[i - 1];
const meter = (m) => `${String(m).replace('.', ',')} m`;

function getal(e, veld, obj = bouwplan) {
  // Een geleegd veld geeft null, en Number(null) is 0: leeg is geen antwoord.
  if (e.target.value === null || e.target.value === '') return;
  const v = Number(e.target.value);
  if (Number.isFinite(v)) obj[veld] = v;
}
</script>

<template>
  <nldd-container layout="stack" gap="16" :class="{ bezig }">
    <nldd-title
      size="display-4" :heading-level="2" overline="Simuleren" text="Wat als het bouwplan of de aannames anders zijn?"
      :supporting-text="cel ? `Vak bij RD ${Math.round(cel.rd[0])}, ${Math.round(cel.rd[1])}. Het DSO-spoor rekent opnieuw, hier in je browser. Verkenning, geen besluit.` : 'Het DSO-spoor rekent opnieuw, hier in je browser. Verkenning, geen besluit.'"
    ></nldd-title>
    <nldd-banner v-if="fout" variant="critical" text="De simulatie kon niet rekenen" :supporting-text="fout" :heading-level="3"></nldd-banner>
    <nldd-text v-else-if="!motor" size="sm" color="secondary"><p>Engine en regels laden…</p></nldd-text>

    <!-- 1. Het bouwplan. -->
    <section class="situatie" aria-label="Situatie">
      <nldd-text size="sm"><p><strong>Situatie: het bouwplan</strong></p></nldd-text>
      <nldd-text size="sm" color="secondary"><p>{{ bouwplan.lagen }} lagen van {{ String(bouwplan.meterPerLaag).replace('.', ',') }} m is {{ meter(hoogte()) }}, {{ bouwplan.aantalWoningen }} woningen.</p></nldd-text>
      <div class="velden">
        <nldd-form-field label="Lagen">
          <nldd-number-field :value="bouwplan.lagen" min="1" max="15" step="1" size="sm" @change="getal($event, 'lagen')"></nldd-number-field>
        </nldd-form-field>
        <nldd-form-field label="Meter per laag">
          <nldd-number-field :value="bouwplan.meterPerLaag" min="2.5" max="5" step="0.1" size="sm" @change="getal($event, 'meterPerLaag')"></nldd-number-field>
        </nldd-form-field>
        <nldd-form-field label="Woningen">
          <nldd-number-field :value="bouwplan.aantalWoningen" min="1" max="500" step="1" size="sm" @change="getal($event, 'aantalWoningen')"></nldd-number-field>
        </nldd-form-field>
      </div>
      <nldd-segmented-control :value="bouwplan.woningtype" size="sm" width="full" accessible-label="Woningtype" @change="bouwplan.woningtype = $event.target.value">
        <nldd-segmented-control-item v-for="t in WONINGTYPES" :key="t.waarde" :value="t.waarde" :text="t.tekst" :selected="bouwplan.woningtype === t.waarde || undefined"></nldd-segmented-control-item>
      </nldd-segmented-control>
      <nldd-text size="xs" color="secondary"><p>Het aantal woningen telt in het DSO-spoor niet mee: de bouwregels zoals gegenereerd lezen het niet (zie scenario W3).</p></nldd-text>
      <div class="situatie-voet">
        <nldd-text size="xs" color="secondary"><p>{{ isStandaard() ? 'Standaard: 5 lagen gestapeld, alle open gegevens gunstig.' : 'Aangepast ten opzichte van de standaard.' }}</p></nldd-text>
        <div class="knoppen">
          <nldd-button v-if="!isStandaard()" size="sm" appearance="neutral-transparent" start-icon="reset" text="Standaard" @click="standaard()"></nldd-button>
          <nldd-button size="sm" appearance="neutral-tinted" start-icon="map" text="Toon de uitkomst op de kaart" @click="emit('kaart')"></nldd-button>
        </div>
      </div>
    </section>

    <!-- 2. Gegevens die de kaart niet weet. -->
    <Uitklap v-model:open="gegevensOpen" titel="Gegevens invullen" sub="Wat de kaart en de tekst niet weten. Standaard alles gunstig, zoals in de vooraf gerekende uitkomst.">
      <nldd-form-field :label="`Overige artikelen die als gegeven tellen (${motor?.aantalGegeven ?? '…'} van ${motor?.aantalArtikelen ?? '…'})`">
        <nldd-segmented-control :value="aannames.gegeven" size="sm" width="full" accessible-label="Overige gegevens" @change="aannames.gegeven = $event.target.value">
          <nldd-segmented-control-item v-for="k in KEUZE" :key="k.waarde" :value="k.waarde" :text="k.tekst" :selected="aannames.gegeven === k.waarde || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </nldd-form-field>
      <nldd-text size="xs" color="secondary"><p>Artikelen die het DSO-spoor niet rekent maar als voldaan aanneemt: archeologie, parkeren, geluid en de meeste regels van provincie, waterschap en rijk. Zet dit op onbekend om te zien hoeveel van de uitkomst op die aanname rust.</p></nldd-text>
      <nldd-form-field label="Welstand (omgevingsplan 22.29)">
        <nldd-segmented-control :value="aannames.welstand" size="sm" width="full" accessible-label="Welstand" @change="aannames.welstand = $event.target.value">
          <nldd-segmented-control-item v-for="k in KEUZE" :key="k.waarde" :value="k.waarde" :text="k.tekst" :selected="aannames.welstand === k.waarde || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </nldd-form-field>
      <nldd-form-field label="Bodem (omgevingsplan 22.29)">
        <nldd-segmented-control :value="aannames.bodem" size="sm" width="full" accessible-label="Bodem" @change="aannames.bodem = $event.target.value">
          <nldd-segmented-control-item v-for="k in KEUZE" :key="k.waarde" :value="k.waarde" :text="k.tekst" :selected="aannames.bodem === k.waarde || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
      </nldd-form-field>
      <nldd-form-field label="Diepte van de fundering (m)">
        <nldd-number-field :value="aannames.funderingsdiepte_m" min="0" max="80" step="1" size="sm" @change="getal($event, 'funderingsdiepte_m', aannames)"></nldd-number-field>
      </nldd-form-field>
      <nldd-text size="xs" color="secondary"><p>In de boringsvrije zone verbiedt de provincie funderen vanaf 30 m (omgevingsverordening 3.51). Dat telt bij "alle overheden".</p></nldd-text>
    </Uitklap>

    <!-- 3. De uitkomst op dit vak. -->
    <nldd-text v-if="!cel" size="sm"><p>Klik op een plek op de kaart om de uitkomst daar te zien.</p></nldd-text>
    <template v-else-if="res">
      <div class="kolommen">
        <nldd-container layout="stack" gap="8">
          <nldd-banner :variant="banner(klasseVan(res)).variant" size="sm" :heading-level="3"
            :text="`Gemeente: ${banner(klasseVan(res)).tekst.toLowerCase()}`"
            :supporting-text="kol?.reden ?? (kol?.deel ? `Bouwregels ${kol.deel.bouwregels}, gebruik ${kol.deel.gebruiksregels}, overige regels ${kol.deel.aanvullend}.` : '')"></nldd-banner>
          <nldd-text size="xs" color="secondary"><p>Vooraf gerekend (standaard): {{ TEKST[standaardKlasse] ?? 'geen' }}.</p></nldd-text>
        </nldd-container>
        <nldd-container layout="stack" gap="8">
          <div v-if="!beoogd" class="rij"><nldd-text size="sm"><p>Alle overheden samen</p></nldd-text><nldd-tag :color="TAG[totaal] ?? 'neutral'" :text="TEKST[totaal] ?? totaal"></nldd-tag></div>
          <div v-for="(v, o) in res.andere" :key="o" class="rij"><nldd-text size="sm"><p>{{ OVERHEID[o] }}</p></nldd-text><nldd-tag :color="TAG[v.uitkomst] ?? 'neutral'" :text="TEKST[v.uitkomst] ?? v.uitkomst ?? 'geen'"></nldd-tag></div>
        </nldd-container>
      </div>
      <nldd-list v-if="res.plichten.length" appearance="simple" accessible-label="Plichten">
        <nldd-list-item v-for="(p, i) in res.plichten" :key="i" size="sm">
          <nldd-text-cell :overline="OVERHEID[p.overheid]" :text="p.tekst" :supporting-text="p.grond"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </template>

    <!-- 4. Per aantal lagen. -->
    <Uitklap v-if="cel" v-model:open="lagenOpen" titel="Uitkomst per aantal lagen" sub="De toets voor 1 tot en met 15 lagen: bij hoeveel lagen slaat een regel om?">
      <nldd-text v-if="!tabel" size="sm" color="secondary"><p>Rekenen…</p></nldd-text>
      <nldd-text v-else-if="tabel.status !== 'gerekend'" size="sm" color="secondary"><p>{{ tabel.reden ?? 'Hier rekent de toets niet.' }}</p></nldd-text>
      <template v-else>
        <div class="legenda" aria-label="Legenda">
          <span v-for="s in ['toegestaan', 'onthouding', 'niet toegestaan']" :key="s" class="leg-item">
            <span class="cel" :class="staat(s).klasse">{{ staat(s).kort }}</span><nldd-text size="xs"><span>{{ staat(s).tekst }}</span></nldd-text>
          </span>
          <span class="leg-item"><span class="cel leeg kantel-voorbeeld"></span><nldd-text size="xs"><span>regel slaat om</span></nldd-text></span>
          <span class="leg-item"><span class="cel leeg vraag-voorbeeld"></span><nldd-text size="xs"><span>{{ bouwplan.lagen }} lagen, de vraag</span></nldd-text></span>
        </div>
        <nldd-segmented-control :value="toon" size="sm" accessible-label="Welke regels" @change="toon = $event.target.value">
          <nldd-segmented-control-item value="omslaan" text="Regels die omslaan" :selected="toon === 'omslaan' || undefined"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="alle" text="Alle regels" :selected="toon === 'alle' || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>
        <div class="tabel-wrap">
          <table class="sim">
            <caption class="sr-only">Uitkomst per aantal lagen</caption>
            <thead>
              <tr>
                <th scope="col" class="regel">Lagen</th>
                <th v-for="(n, i) in scan.lagen" :key="n" scope="col" :class="{ vraag: n === bouwplan.lagen }">{{ n }}<span class="m">{{ meter(scan.hoogtes[i]) }}</span></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="r in tabel.rijen" :key="r.vorm + r.sleutel" :class="{ eind: r.vorm === 'HUB' || r.vorm === 'TOTAAL' }">
                <th scope="row" class="regel">
                  <span class="art">{{ r.vorm === 'TOTAAL' ? 'Gemeente, provincie, waterschap, rijk' : label(r.sleutel) }}</span>{{ r.vorm === 'TOTAAL' ? 'Alle overheden samen' : LEESBAAR[r.vorm] }}
                </th>
                <td v-for="(u, i) in r.uitkomsten" :key="i" :class="{ vraag: scan.lagen[i] === bouwplan.lagen }">
                  <span class="cel" :class="[staat(u).klasse, { groot: r.vorm === 'HUB' || r.vorm === 'TOTAAL', kantel: isKantel(r, i) }]" :title="`${scan.lagen[i]} lagen: ${staat(u).tekst}`">{{ staat(u).kort }}</span>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <nldd-text v-if="toon === 'omslaan' && tabel.verborgen" size="xs" color="secondary"><p>{{ tabel.verborgen }} regels slaan niet om en staan onder "Alle regels".</p></nldd-text>
        <nldd-list v-if="tabel.kantel.length" appearance="simple" accessible-label="Kantelpunten">
          <nldd-list-item v-for="(p, i) in tabel.kantel" :key="i" size="sm">
            <nldd-text-cell :overline="p.eind ? 'Eindoordeel' : 'Kantelpunt'" :text="p.tekst" :supporting-text="p.regel"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-text v-else size="sm" color="secondary"><p>Geen regel slaat om tussen 1 en 15 lagen.</p></nldd-text>
      </template>
    </Uitklap>
  </nldd-container>
</template>

<style scoped>
.bezig { opacity: 0.6; transition: opacity 0.15s; }
.situatie { display: flex; flex-direction: column; gap: 8px; }
.situatie-voet { display: flex; flex-wrap: wrap; gap: 8px 12px; align-items: center; justify-content: space-between; }
.knoppen { display: flex; gap: 6px; flex-wrap: wrap; }
.velden { display: grid; grid-template-columns: repeat(auto-fit, minmax(130px, 1fr)); gap: 8px; }
.kolommen { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; align-items: start; }
@container paneel (max-width: 620px) { .kolommen { grid-template-columns: 1fr; } }
.rij { display: flex; align-items: center; justify-content: space-between; gap: 8px; }

/* Statuskleuren en tabel: gelijk aan de Simuleren-tab van de eerste PoC. */
.goed { --status: #0ca30c; }
.open { --status: #fab219; }
.kritiek { --status: #d03b3b; }
.legenda { display: flex; flex-wrap: wrap; gap: 6px 14px; align-items: center; }
.leg-item { display: inline-flex; align-items: center; gap: 6px; }
.tabel-wrap { overflow-x: auto; }
.sim { border-collapse: separate; border-spacing: 2px; font-size: 0.78rem; }
.sim th, .sim td { padding: 0; text-align: center; vertical-align: middle; }
.sim thead th { font-weight: 600; padding: 0 0 4px; min-width: 28px; }
.sim .m { display: block; font-weight: 400; font-size: 0.66rem; opacity: 0.65; }
.sim .regel {
  text-align: left; font-weight: 400; padding-right: 8px; min-width: 150px; max-width: 220px;
  position: sticky; left: 0; z-index: 1;
  background: var(--semantics-surfaces-base-background-color, Canvas);
}
.sim .art { display: block; font-size: 0.66rem; opacity: 0.65; }
.sim tr.eind .regel { font-weight: 700; }
.cel {
  display: inline-flex; align-items: center; justify-content: center;
  width: 28px; height: 24px; border-radius: 4px; font-size: 0.72rem; line-height: 1;
  background: color-mix(in srgb, var(--status, transparent) 26%, Canvas);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--status, #999) 55%, transparent);
  color: CanvasText;
}
.cel.groot { height: 30px; font-weight: 700; background: color-mix(in srgb, var(--status, transparent) 40%, Canvas); }
.cel.leeg { box-shadow: inset 0 0 0 1px color-mix(in srgb, #999 40%, transparent); opacity: 0.6; }
.cel.kantel-voorbeeld { opacity: 1; }
.cel.kantel, .cel.kantel-voorbeeld { box-shadow: inset 3px 0 0 0 CanvasText, inset 0 0 0 1px color-mix(in srgb, var(--status, #999) 55%, transparent); }
td.vraag, th.vraag { outline: 2px solid #154273; outline-offset: 0; border-radius: 5px; }
.vraag-voorbeeld { outline: 2px solid #154273; }
@media (prefers-color-scheme: dark) {
  td.vraag, th.vraag, .vraag-voorbeeld { outline-color: #8fb8de; }
}
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
</style>
