<template>
  <div class="bestand">
    <nldd-tab-bar v-if="sector === 'po'" variant="text" accessible-label="Tabbladen Bestand Nieuwkomers" @tab-change="onTab">
      <nldd-tab-bar-item
        v-for="t in [1, 2, 3]"
        :key="t"
        :text="`${TABBLAD_LABELS[t]} (${perTabblad[t].length})`"
        :data-tab="t"
        :current="tab === t ? true : undefined"
      ></nldd-tab-bar-item>
    </nldd-tab-bar>
    <p class="bs-uitleg">{{ uitleg }}</p>

    <nldd-table
      columns="minmax(150px, 1.2fr) 110px 90px minmax(150px, 1fr) 120px minmax(130px, 1fr) 110px"
      accessible-label="Leerlingen in het Bestand Nieuwkomers"
    >
      <nldd-inline-dialog slot="empty" text="Geen leerlingen op dit tabblad" supporting-text="Het bestand toont leerlingen tot twee jaar na de eerste inschrijving."></nldd-inline-dialog>
      <nldd-table-row slot="header">
        <nldd-text-cell size="sm" text="Leerling"></nldd-text-cell>
        <nldd-text-cell size="sm" text="Geboren"></nldd-text-cell>
        <nldd-text-cell size="sm" text="Code"></nldd-text-cell>
        <nldd-text-cell size="sm" text="Categorie"></nldd-text-cell>
        <nldd-text-cell size="sm" text="1e inschrijving"></nldd-text-cell>
        <nldd-text-cell size="sm" text="Telt"></nldd-text-cell>
        <nldd-text-cell size="sm" text="Per kwartaal" horizontal-alignment="right"></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="l in zichtbaar" :key="l.record.bsn">
        <nldd-text-cell size="sm" :text="l.record.naam || l.record.bsn" :supporting-text="l.record.naam ? l.record.bsn : (l.record.heeft_bsn === false ? 'onderwijsnummer' : '')"></nldd-text-cell>
        <nldd-text-cell size="sm" :text="datumKort(l.record.geboortedatum)"></nldd-text-cell>
        <nldd-text-cell size="sm" :text="l.record.heeft_bsn === false ? 'geen BSN' : String(l.record.verblijfstitel_code ?? '—')"></nldd-text-cell>
        <nldd-text-cell size="sm">
          <nldd-tag size="sm" :color="categorieColor(l.uitkomst.categorie)" :text="categorieLabel(l.uitkomst.categorie)"></nldd-tag>
          <span v-if="l.uitkomst.categorie === 'BESTUUR_BEOORDEELT'" slot="supporting-text">
            oordeel: {{ l.record.oordeel_bevoegd_gezag ? categorieLabel(l.record.oordeel_bevoegd_gezag) : 'nog te bepalen' }}
          </span>
        </nldd-text-cell>
        <nldd-text-cell size="sm" :text="datumKort(l.record.eerste_inschrijfdatum)"></nldd-text-cell>
        <nldd-text-cell size="sm" :color="l.uitkomst.telt ? 'success' : 'secondary'" :text="l.uitkomst.telt ? `ja, jaar ${l.uitkomst.bekostigingsjaar}` : 'nee'" :supporting-text="teltUitleg(l.uitkomst)"></nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right" :text="l.uitkomst.telt ? euro(l.uitkomst.bedrag) : '—'"></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue';
import { euro } from '../../lib/format.js';
import { datumKort, categorieLabel, categorieColor, TABBLAD_LABELS } from '../../lib/nieuwkomerFacts.js';

const props = defineProps({
  /** [{ record, uitkomst, tabblad }] van leerlingen in het bestand op de peildatum */
  leerlingen: { type: Array, required: true },
  sector: { type: String, default: 'po' },
});

const tab = ref(1);

const perTabblad = computed(() => {
  const out = { 1: [], 2: [], 3: [] };
  for (const l of props.leerlingen) out[l.tabblad ?? 1]?.push(l);
  return out;
});

const zichtbaar = computed(() => (props.sector === 'po' ? perTabblad.value[tab.value] : props.leerlingen));

const uitleg = computed(() => {
  if (props.sector !== 'po') {
    return 'DUO leest op de zestiende na de peildatum uit ROD welke leerlingen nieuwkomer zijn en stelt de bekostiging ambtshalve vast; alleen bij een onderwijsnummer levert de school een bewijsstuk.';
  }
  return {
    1: 'Eenduidige verblijfstitelcode: DUO bepaalt de categorie; het bestuur controleert en telt.',
    2: 'Code waarbij het bevoegd gezag beslist (art. 34 lid 11): het bestuur bepaalt zelf of dit een asielzoeker of overige vreemdeling is.',
    3: 'Ingeschreven op onderwijsnummer (geen BSN): het bestuur beoordeelt alles zelf en levert bewijs.',
  }[tab.value];
});

function onTab(event) {
  const item = event.detail?.item;
  const t = Number(item?.dataset?.tab ?? item?.getAttribute?.('data-tab'));
  if (t) tab.value = t;
}

function teltUitleg(u) {
  if (u.telt) return '';
  if (u.bekostigbare_kwartalen != null && u.kwartalen_verstreken != null && u.kwartalen_verstreken >= u.bekostigbare_kwartalen) {
    return `${u.bekostigbare_kwartalen} kwartalen verbruikt`;
  }
  if (u.categorie === 'GEEN') return 'geen nieuwkomer';
  return '';
}
</script>

<style scoped>
.bestand { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.bs-uitleg { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
</style>
