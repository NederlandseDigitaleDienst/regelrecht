<script setup>
import { computed, onMounted, ref } from 'vue';
import PortalHeader from '../components/PortalHeader.vue';
import NBanner from '../components/NBanner.vue';
import { api } from '../api.js';
import { euro, datum } from '../format.js';
import { b } from '../basePad.js';

const navItems = [
  { text: 'Home', to: '/' },
  { text: 'Openbaar register', to: '/register' },
];

const entries = ref([]);
const stats = ref(null);
const fout = ref('');

const maxMaandBedrag = computed(() =>
  Math.max(1, ...(stats.value?.per_maand ?? []).map((m) => m.toegekend_bedrag)),
);

onMounted(async () => {
  try {
    [entries.value, stats.value] = await Promise.all([api.register(), api.statistieken()]);
  } catch (e) {
    fout.value = String(e?.message ?? e);
  }
});
</script>

<template>
  <nldd-page>
    <PortalHeader
      slot="header"
      portal="publiek"
      :items="navItems"
    />

    <nldd-simple-section>
      <nldd-title size="1">
        <span slot="overline">Openbaar register</span>
        <h1>Subsidiebesluiten politieke partijen</h1>
      </nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-rich-text>
        <p>
          Alle bekendgemaakte subsidiebesluiten van de Nederlandse autoriteit
          politieke partijen, met statistieken over aanvragen en toekenningen.
        </p>
      </nldd-rich-text>
    </nldd-simple-section>

    <nldd-simple-section v-if="stats" background="tinted" padding-block="32">
      <nldd-collection layout="grid" item-min-width="220px">
        <nldd-card accessible-label="Aantal aanvragen">
          <nldd-container padding="20">
            <nldd-text-cell overline="Aanvragen" :text="String(stats.aantal_aanvragen)" size="md"></nldd-text-cell>
          </nldd-container>
        </nldd-card>
        <nldd-card accessible-label="Aantal toegekend">
          <nldd-container padding="20">
            <nldd-text-cell overline="Toegekend" :text="String(stats.aantal_toegekend)" size="md"></nldd-text-cell>
          </nldd-container>
        </nldd-card>
        <nldd-card accessible-label="Aantal afgewezen">
          <nldd-container padding="20">
            <nldd-text-cell overline="Afgewezen" :text="String(stats.aantal_afgewezen)" size="md"></nldd-text-cell>
          </nldd-container>
        </nldd-card>
        <nldd-card accessible-label="Totaal toegekend bedrag">
          <nldd-container padding="20">
            <nldd-text-cell overline="Totaal toegekend" :text="euro(stats.totaal_toegekend_bedrag)" size="md"></nldd-text-cell>
          </nldd-container>
        </nldd-card>
      </nldd-collection>
    </nldd-simple-section>

    <nldd-simple-section>
      <nldd-title size="3" slot="header"><h2>Register</h2></nldd-title>

      <NBanner v-if="fout" variant="critical" text="Register laden mislukt" :supporting-text="fout" />

      <nldd-table
        v-else
        columns="minmax(200px,1.4fr) 90px 120px 130px 160px 170px"
        sm-columns="minmax(160px,1fr) 130px"
        accessible-label="Openbaar register van subsidiebesluiten"
      >
        <nldd-inline-dialog slot="empty" text="Nog geen bekendgemaakte besluiten" supporting-text="Besluiten verschijnen hier zodra ze door de Napp zijn bekendgemaakt."></nldd-inline-dialog>
        <nldd-table-row slot="header">
          <nldd-text-cell text="Partij"></nldd-text-cell>
          <nldd-text-cell text="Jaar" hide-below="md"></nldd-text-cell>
          <nldd-text-cell text="Onderdelen" horizontal-alignment="right" hide-below="md"></nldd-text-cell>
          <nldd-text-cell text="Besluit"></nldd-text-cell>
          <nldd-text-cell text="Bedrag" horizontal-alignment="right" hide-below="md"></nldd-text-cell>
          <nldd-text-cell text="Bekendgemaakt" hide-below="md"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="(entry, i) in entries" :key="i">
          <nldd-text-cell :text="entry.partij_naam"></nldd-text-cell>
          <nldd-text-cell :text="String(entry.subsidiejaar)" hide-below="md"></nldd-text-cell>
          <nldd-text-cell :text="String(entry.aantal_componenten)" horizontal-alignment="right" hide-below="md"></nldd-text-cell>
          <nldd-cell>
            <nldd-tag
              :color="entry.subsidie_toegekend ? 'success' : 'critical'"
              :text="entry.subsidie_toegekend ? 'Toegekend' : 'Afgewezen'"
            ></nldd-tag>
          </nldd-cell>
          <nldd-text-cell
            :text="entry.subsidie_toegekend ? euro(entry.subsidiebedrag) : '—'"
            horizontal-alignment="right"
            hide-below="md"
          ></nldd-text-cell>
          <nldd-text-cell :text="datum(entry.bekendmaking_datum)" color="secondary" hide-below="md"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
    </nldd-simple-section>

    <nldd-simple-section v-if="stats?.per_maand?.length">
      <nldd-title size="3" slot="header"><h2>Toegekend bedrag per maand</h2></nldd-title>
      <nldd-container gap="12">
        <nldd-progress-bar
          v-for="maand in stats.per_maand"
          :key="maand.maand"
          variant="progress"
          :max="maxMaandBedrag"
          :value="maand.toegekend_bedrag"
          :text="maand.maand"
          :value-text="`${euro(maand.toegekend_bedrag)} · ${maand.aantal_aanvragen} aanvragen`"
          color="lintblauw"
          size="md"
        ></nldd-progress-bar>
      </nldd-container>
    </nldd-simple-section>

    <nldd-page-footer>
      <nldd-container padding="24">
        <nldd-rich-text>
          <p>
            Dit register wordt bijgehouden door de Nederlandse autoriteit politieke
            partijen op grond van de Wet op de politieke partijen.
          </p>
        </nldd-rich-text>
      </nldd-container>
      <nldd-page-footer-legal-bar slot="legal-bar">
        <nldd-page-footer-legal-bar-item :href="b('/')" text="Home"></nldd-page-footer-legal-bar-item>
        <nldd-page-footer-legal-bar-item href="#" text="Contact"></nldd-page-footer-legal-bar-item>
        <nldd-page-footer-legal-bar-item href="#" text="Toegankelijkheid"></nldd-page-footer-legal-bar-item>
        <nldd-page-footer-legal-bar-item href="#" text="Privacy"></nldd-page-footer-legal-bar-item>
      </nldd-page-footer-legal-bar>
    </nldd-page-footer>
  </nldd-page>
</template>
