<script setup>
// Wat het besluit zou worden, voordat het bestuursorgaan het neemt. Het
// spel neemt het besluit op een kopie van de cel (Cell::sandbox) en
// legt niets vast; de uitkomst en de keten zijn die van het echte besluit.
// Na elke wijziging van een bevinding vraagt het paneel opnieuw, 300 ms na
// de laatste (voorvertoning.js). Weigert de cel, bijvoorbeeld omdat een
// bevinding nog ontbreekt, dan staat dat hier in plaats van een uitkomst.
// Mislukt het verzoek zelf, dan zegt het paneel dat, met de melding erbij.
// Bij een bezwaar (`soort="bezwaar"`) is het de beslissing op bezwaar: of
// het bezwaar gegrond is en welk nieuw besluit er dan komt; zo ook bij
// opnieuw beslissen op een bezwaar na een uitspraak (`soort="na-uitspraak"`,
// RFC-053). Bij `soort="uitspraak"` is het de uitspraak van de rechter, op
// een kopie van haar cel. Met elk
// antwoord geeft het paneel door welke begrippen de cel voor dit geval niet
// weet (`stil`): die beoordeelt het bestuursorgaan, en het formulier vraagt
// ze dan.
import { computed, onUnmounted, ref, watch } from 'vue';
import { api } from '../api.js';
import { beroepApi } from '../beroep.js';
import { genummerd, toon, uitslag } from '../logboek.js';
import { vertaling } from '../melding.js';
import { hoofdletter, leesbaar, state, toegewezen } from '../state.js';
import { volger } from '../voorvertoning.js';
import KetenStap from './KetenStap.vue';

const props = defineProps({
  soort: { type: String, default: 'besluit' },
  // De aanvraag, het bezwaar, de ontvangst van een uitspraak of het beroep
  // waarop beslist wordt.
  op: { type: String, required: true },
  bevindingen: { type: Object, required: true },
});

// null zolang het eerste antwoord er niet is; daarna { antwoord } of { fout }.
const stand = ref(null);
// Of er een nieuwere vraag loopt dan het antwoord dat er staat.
const bezig = ref(false);
const VRAAG = {
  besluit: api.voorvertoningBesluit,
  bezwaar: api.voorvertoningOpBezwaar,
  'na-uitspraak': beroepApi.voorvertoningNaUitspraak,
  uitspraak: beroepApi.voorvertoningUitspraak,
};
const vraag = () => VRAAG[props.soort](props.op, { ...props.bevindingen }, state.datum);
// Een beslissing op bezwaar, met misschien een nieuw besluit.
const opBezwaarSoort = computed(() => props.soort === 'bezwaar' || props.soort === 'na-uitspraak');
const emit = defineEmits(['stil']);
const volg = volger(vraag, (uit) => {
  stand.value = uit;
  bezig.value = false;
  if (uit.antwoord?.stil?.length) emit('stil', uit.antwoord.stil);
});
watch(
  () => [
    props.op,
    JSON.stringify(props.bevindingen),
    state.datum,
    state.grammen.length,
    state.rechter.length,
  ],
  () => {
    bezig.value = true;
    volg.plan();
  },
  { immediate: true },
);
onUnmounted(() => volg.stop());

const voorlopig = computed(() => stand.value?.antwoord?.voorlopig ?? null);
const geweigerd = computed(() => stand.value?.antwoord?.geweigerd ?? null);
const mislukt = computed(() => stand.value?.fout ?? null);
// De reden in het Nederlands; zonder vertaling de melding zelf.
const reden = (fout) => vertaling(fout) ?? fout;
// Het besluit op de aanvraag, of het nieuwe besluit na een gegrond bezwaar.
const besluit = computed(() =>
  opBezwaarSoort.value ? voorlopig.value?.nieuw_besluit : voorlopig.value?.besluit,
);
const beslissing = computed(() => (opBezwaarSoort.value ? voorlopig.value?.beslissing : null));
const uitspraak = computed(() => voorlopig.value?.uitspraak ?? null);
const keten = computed(() => genummerd(voorlopig.value?.waarom?.keten ?? []));

/** De uitkomst als badge: de uitvoer die de casus noemt, met ja of nee. */
function badge(b) {
  const t = toegewezen(b);
  return {
    tekst: `${hoofdletter(leesbaar(state.casus.uitkomst))}: ${toon(t)}`,
    kleur: t === true ? 'success' : t === false ? 'critical' : 'neutral',
  };
}
const opBezwaar = computed(() => {
  if (!beslissing.value) return null;
  const u = uitslag(beslissing.value);
  return { tekst: hoofdletter(u.tekst), kleur: u.kleur };
});
// Wat een beslissing op bezwaar of een uitspraak uitrekende, in woorden.
const regels = (g) =>
  Object.entries(g?.outputs ?? {})
    .filter(([, v]) => typeof v === 'boolean')
    .map(([naam, v]) => `${leesbaar(naam)}: ${toon(v)}`)
    .join(' · ');
</script>

<template>
  <nldd-card background="tinted" accessible-label="Voorlopige uitkomst">
    <nldd-container padding="16" gap="12">
      <nldd-title
        size="5"
        text="Voorlopige uitkomst · nog niet vastgelegd"
        :supporting-text="bezig && stand !== null ? 'Wordt bijgewerkt…' : `Zo zou ${soort === 'uitspraak' ? 'de uitspraak' : 'het besluit'} uitvallen met de bevindingen zoals ze nu staan. Het loopt mee met elke wijziging; pas de knop legt het vast.`"
      ></nldd-title>
      <nldd-inline-dialog
        v-if="stand === null"
        variant="loading"
        text="De uitkomst wordt berekend"
      ></nldd-inline-dialog>
      <nldd-inline-dialog
        v-else-if="mislukt"
        icon="warning"
        icon-color="critical"
        text="De voorlopige uitkomst lukt niet"
        :supporting-text="reden(mislukt)"
      ></nldd-inline-dialog>
      <nldd-inline-dialog
        v-else-if="geweigerd"
        icon="info"
        text="Nog geen uitkomst"
        :supporting-text="reden(geweigerd)"
      ></nldd-inline-dialog>
      <template v-else-if="voorlopig">
        <!-- Een tussenuitspraak strekt nog nergens toe (Awb 8:80a). -->
        <nldd-title
          v-if="uitspraak"
          size="6"
          :text="uitspraak.outputs.tussenuitspraak ? 'Tussenuitspraak' : 'Uitspraak'"
          :supporting-text="regels(uitspraak)"
        >
          <nldd-badge
            slot="end"
            size="sm"
            color="neutral"
            :text="uitspraak.outputs.tussenuitspraak ? 'Gelegenheid tot herstel' : hoofdletter(toon(uitspraak.outputs.uitspraak_strekt_tot))"
          ></nldd-badge>
        </nldd-title>
        <nldd-title v-if="opBezwaar" size="6" text="Beslissing op bezwaar" :supporting-text="regels(beslissing)">
          <nldd-badge slot="end" size="sm" :color="opBezwaar.kleur" :text="opBezwaar.tekst"></nldd-badge>
        </nldd-title>
        <nldd-title
          v-if="besluit"
          size="6"
          :text="opBezwaarSoort ? 'Nieuw besluit' : 'Besluit'"
        >
          <nldd-badge slot="end" size="sm" :color="badge(besluit).kleur" :text="badge(besluit).tekst"></nldd-badge>
        </nldd-title>
        <nldd-text v-else-if="opBezwaarSoort" size="sm" color="secondary">
          Er komt geen nieuw besluit: het bestreden besluit blijft.
        </nldd-text>
        <nldd-list
          v-if="keten.length"
          type="tree"
          appearance="box-base"
          accessible-label="Keten van de voorlopige uitkomst"
        >
          <KetenStap v-for="k in keten" :key="k.nummer" :knoop="k" />
        </nldd-list>
      </template>
    </nldd-container>
  </nldd-card>
</template>
