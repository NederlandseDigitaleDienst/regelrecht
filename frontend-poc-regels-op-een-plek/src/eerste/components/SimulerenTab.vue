<script setup>
// Tab Simuleren in het kaartpaneel. Opbouw: eerst de situatie
// (punt en bouwplan), dan twee blokken die je in- en uitklapt: de uitkomst per aantal lagen
// (1 tot en met 15, per regel waar hij omslaat) en de gegevens die de kaart niet kent. Status in
// kleur én tekst (ja, ?, nee), nooit alleen kleur. Deze component rekent niets; zie
// toets/simuleer.js.
import { computed, nextTick, ref, watch } from 'vue';
import AannamesInvoer from './AannamesInvoer.vue';
import BouwplanInvoer from './BouwplanInvoer.vue';
import Uitklap from './Uitklap.vue';
import { maakEngine } from '../engine.js';
import { bouwplan } from '../toets/bouwplan.js';
import { VELDEN, aannames, aantalIngevuld, naarParams } from '../toets/aannames.js';
import { laadRegelingen } from '../toets/laden.js';
import { KORT } from '../toets/plannen.js';
import { kantelpunten, simuleer } from '../toets/simuleer.js';

const props = defineProps({
  stapel: { type: Object, default: null },
  rd: { type: Array, default: null },
});
// uitleg: open de tab Uitleg bij een begrip. kaart: toon de kaartlaag Uitkomst.
const emit = defineEmits(['uitleg', 'kaart']);
// De kaart kan vragen om "Gegevens invullen" te openen (deel = 'invoer'); daarna weer leeg.
const deel = defineModel('deel', { default: '' });
const ingevuld = computed(() => aantalIngevuld());
const lagenOpen = ref(true);
const invoerOpen = ref(false);
const invoerEl = ref(null);
watch(deel, async (d) => {
  if (d !== 'invoer') return;
  // Vanuit de kaart ("Wijzigen"): de invoer in beeld, de tabel per laag even dicht.
  invoerOpen.value = true;
  lagenOpen.value = false;
  deel.value = '';
  await nextTick();
  invoerEl.value?.$el?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}, { immediate: true });

const fout = ref(null);
const bezig = ref(false);
const res = ref(null);
const toon = ref('omslaan');
const vandaag = new Date().toISOString().slice(0, 10);

async function reken() {
  if (!props.stapel) { res.value = null; return; }
  bezig.value = true;
  try {
    const { index, varianten } = await laadRegelingen();
    const { lagen, ...rest } = bouwplan; // het aantal lagen schuift; de rest blijft
    res.value = await simuleer({ stapel: props.stapel, bouwplan: rest, aannames: naarParams(), datum: vandaag, index, varianten, maakEngine });
    fout.value = null;
  } catch (e) {
    fout.value = String(e?.message ?? e);
  } finally {
    bezig.value = false;
  }
}
// Het aantal lagen schuift in de tabel; al het andere rekent opnieuw.
watch([() => props.stapel, () => { const { lagen, ...rest } = bouwplan; return rest; }, () => ({ ...aannames })], reken, { deep: true, immediate: true });

const KOLOM = { tijdelijk: 'Tijdelijk deel', nieuw: 'Nieuw deel' };
const STAAT = {
  toegestaan: { kort: 'ja', klasse: 'goed', tekst: 'toegestaan' },
  'niet toegestaan': { kort: 'nee', klasse: 'kritiek', tekst: 'niet toegestaan' },
  onthouding: { kort: '?', klasse: 'open', tekst: 'onthouding' },
  fout: { kort: '!', klasse: 'kritiek', tekst: 'fout' },
};
const staat = (u) => STAAT[u] ?? { kort: '·', klasse: 'leeg', tekst: u };
const STATUS = {
  'niet-van-toepassing': 'Niet van toepassing: op dit punt geldt een vastgesteld TAM-plan. De regels staan in het nieuwe deel.',
  'bron-ontbreekt': 'Onthouding: het plan zit niet in de bronnen, of alleen als scan.',
  'geen-regels': 'Geen bouwregels in het nieuwe deel: afdeling 5.3 is gereserveerd, en er ligt hier geen TAM-plan.',
  'geen-plan': 'Geen plan gevonden op dit punt.',
  'geen-bestemming': 'Op dit punt ligt geen bestemming: een naad tussen twee vlakken in de GML.',
};

const leesbaar = (naam) => naam.replaceAll('_', ' ');
const meter = (m) => String(m).replace('.', ',');

const kolommen = computed(() => {
  if (!res.value) return [];
  return ['tijdelijk', 'nieuw'].map((naam) => {
    const k = res.value[naam];
    if (k.status !== 'gerekend') return { naam, ...k };
    const eind = k.rijen.find((r) => r.eindoordeel);
    const regels = k.rijen.filter((r) => !r.eindoordeel && (toon.value === 'alle' || kantelpunten(r).length));
    const kantel = k.rijen.flatMap((r) => kantelpunten(r).map((p) => ({
      eind: r.eindoordeel,
      regel: `${KORT[r.regeling] ?? r.regeling}, artikel ${r.artikel}: ${leesbaar(r.output)}`,
      tekst: `van ${p.van} naar ${p.naar} bij ${res.value.lagen[p.index]} lagen (${meter(res.value.hoogtes[p.index])} m)`,
    })));
    return { naam, ...k, eind, regels, kantel, verborgen: k.rijen.length - 1 - regels.length };
  });
});

const isKantel = (rij, i) => i > 0 && rij.uitkomsten[i] !== rij.uitkomsten[i - 1];
const titel = (rij, i) => `${res.value.lagen[i]} lagen (${meter(res.value.hoogtes[i])} m): ${staat(rij.uitkomsten[i]).tekst}`;
</script>

<template>
  <nldd-container layout="stack" gap="16" :class="{ bezig }">
    <nldd-title
      size="display-4"
      :heading-level="2"
      overline="Simuleren"
      text="Wat als het bouwplan anders is?"
      :supporting-text="rd ? `Punt RD ${Math.round(rd[0])}, ${Math.round(rd[1])}. Verkenning, geen besluit.` : ''"
    ></nldd-title>
    <nldd-text v-if="!stapel" size="sm"><p>Klik op een plek op de kaart, of kies een testpunt.</p></nldd-text>

    <!-- 1. De situatie: het bouwplan. Altijd zichtbaar; de rest rekent ermee. -->
    <section class="situatie" aria-label="Situatie">
      <nldd-text size="sm"><p><strong>Situatie: het bouwplan</strong></p></nldd-text>
      <BouwplanInvoer alles />
      <div class="situatie-voet">
        <nldd-text size="xs" color="secondary">
          <p>
            {{ ingevuld ? `Met ${ingevuld} aannames uit "Gegevens invullen".` : 'Nog geen aannames: wat de kaart niet weet, is onbekend.' }}
          </p>
        </nldd-text>
        <nldd-button size="sm" appearance="neutral-tinted" start-icon="map" text="Toon de uitkomst op de kaart" @click="emit('kaart')"></nldd-button>
      </div>
    </section>

    <nldd-banner v-if="fout" variant="critical" text="De simulatie kon niet rekenen" :supporting-text="fout" :heading-level="3"></nldd-banner>

    <!-- 2. Uitkomst per aantal lagen. -->
    <Uitklap v-model:open="lagenOpen" titel="Uitkomst per aantal lagen" sub="De toets voor 1 tot en met 15 lagen: bij hoeveel lagen slaat een regel om?">
      <nldd-text v-if="!stapel || !res" size="sm" color="secondary"><p>{{ stapel ? 'Rekenen…' : 'Kies eerst een plek.' }}</p></nldd-text>
      <template v-else>
        <div class="legenda" aria-label="Legenda">
          <span v-for="s in ['toegestaan', 'onthouding', 'niet toegestaan']" :key="s" class="leg-item">
            <span class="cel" :class="staat(s).klasse">{{ staat(s).kort }}</span>
            <nldd-text size="xs"><span>{{ staat(s).tekst }}</span></nldd-text>
          </span>
          <span class="leg-item"><span class="cel leeg kantel-voorbeeld"></span><nldd-text size="xs"><p>regel slaat om</p></nldd-text></span>
          <span class="leg-item"><span class="cel leeg vraag-voorbeeld"></span><nldd-text size="xs"><span>{{ bouwplan.lagen }} lagen, de vraag</span></nldd-text></span>
          <a href="#" class="uitleg-link" @click.prevent="emit('uitleg', 'onthouding')"><nldd-text size="xs"><p>Wat betekenen deze termen?</p></nldd-text></a>
        </div>

        <nldd-segmented-control :value="toon" size="sm" accessible-label="Welke regels" @change="toon = $event.target.value">
          <nldd-segmented-control-item value="omslaan" text="Regels die omslaan" :selected="toon === 'omslaan' || undefined"></nldd-segmented-control-item>
          <nldd-segmented-control-item value="alle" text="Alle regels" :selected="toon === 'alle' || undefined"></nldd-segmented-control-item>
        </nldd-segmented-control>

        <nldd-container v-for="k in kolommen" :key="k.naam" layout="stack" gap="8">
          <nldd-title
            size="display-5"
            :heading-level="3"
            :text="KOLOM[k.naam]"
            :overline="k.label === 'beoogd' ? 'Beoogd: ontwerp, geen geldend recht' : (k.label ? 'Geldend' : '')"
            :supporting-text="k.plan ? k.plan.naam : ''"
          ></nldd-title>
          <nldd-text v-if="k.status !== 'gerekend'" size="sm" color="secondary"><p>{{ STATUS[k.status] ?? k.toelichting }}</p></nldd-text>

          <template v-else>
            <div class="tabel-wrap">
              <table class="sim">
                <caption class="sr-only">{{ KOLOM[k.naam] }}: uitkomst per aantal lagen</caption>
                <thead>
                  <tr>
                    <th scope="col" class="regel">Lagen</th>
                    <th v-for="(n, i) in res.lagen" :key="n" scope="col" :class="{ vraag: n === bouwplan.lagen }">
                      {{ n }}<span class="m">{{ meter(res.hoogtes[i]) }}</span>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-if="k.eind" class="eind">
                    <th scope="row" class="regel"><span class="art">Omgevingsplan, artikel 22.29</span>Eindoordeel</th>
                    <td v-for="(u, i) in k.eind.uitkomsten" :key="i" :class="{ vraag: res.lagen[i] === bouwplan.lagen }">
                      <span class="cel groot" :class="[staat(u).klasse, { kantel: isKantel(k.eind, i) }]" :title="titel(k.eind, i)">{{ staat(u).kort }}</span>
                    </td>
                  </tr>
                  <tr v-for="r in k.regels" :key="`${r.regeling}|${r.artikel}|${r.output}`">
                    <th scope="row" class="regel"><span class="art">{{ KORT[r.regeling] ?? r.regeling }}, art. {{ r.artikel }}</span>{{ leesbaar(r.output) }}</th>
                    <td v-for="(u, i) in r.uitkomsten" :key="i" :class="{ vraag: res.lagen[i] === bouwplan.lagen }">
                      <span class="cel" :class="[staat(u).klasse, { kantel: isKantel(r, i) }]" :title="titel(r, i)">{{ staat(u).kort }}</span>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
            <nldd-text v-if="toon === 'omslaan' && k.verborgen" size="xs" color="secondary"><p>{{ k.verborgen }} regels slaan niet om en staan onder "Alle regels".</p></nldd-text>

            <nldd-list v-if="k.kantel.length" appearance="simple" accessible-label="Kantelpunten">
              <nldd-list-item v-for="(p, i) in k.kantel" :key="i" size="sm">
                <nldd-text-cell :overline="p.eind ? 'Eindoordeel' : 'Kantelpunt'" :text="p.tekst" :supporting-text="p.regel"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <nldd-text v-else size="sm" color="secondary"><p>Geen regel slaat om tussen 1 en 15 lagen.</p></nldd-text>
          </template>
        </nldd-container>
      </template>
    </Uitklap>

    <!-- 3. Gegevens die de kaart niet kent. -->
    <Uitklap
      ref="invoerEl"
      v-model:open="invoerOpen"
      titel="Gegevens invullen"
      :sub="`Wat de kaart niet weet: oordelen, onderzoeken, perceel. ${ingevuld} van ${VELDEN.length} ingevuld.`"
    >
      <AannamesInvoer :spelen="res?.spelen ?? new Set()" @uitleg="emit('uitleg', $event)" />
    </Uitklap>
  </nldd-container>
</template>

<style scoped>
.bezig { opacity: 0.6; transition: opacity 0.15s; }
.situatie { display: flex; flex-direction: column; gap: 8px; }
.situatie-voet { display: flex; flex-wrap: wrap; gap: 8px 12px; align-items: center; justify-content: space-between; }

/* Statuskleuren (dataviz-palet): goed, waarschuwing, kritiek; altijd met tekst in de cel. */
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
