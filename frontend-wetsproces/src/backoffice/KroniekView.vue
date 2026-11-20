<script setup lang="ts">
// The chronicle of a cell the employee may read: every record, in the order
// it was recorded. Nothing in it changes or goes away. Laid out as the
// editor's library: the cells in the sidebar, the chronicle of the chosen one
// in the main pane beside it.
import { computed, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { omschrijving } from '../gedeeld/keten';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel?: string }>();
const router = useRouter();
const { casus, kronieken, fout, laden, laad } = useBackoffice();
onMounted(laad);

const gekozen = computed(() => kronieken.value.find((k) => k.cel === props.cel));
const kies = (cel: string) => router.push({ name: 'kroniek', params: { cel } });
</script>

<template>
  <nldd-navigation-split-view @back="router.push('/backoffice/kroniek')">
    <nldd-split-view-pane slot="primary-sidebar" has-content>
      <nldd-page sticky-header accessible-label="Kroniek">
        <nldd-top-title-bar slot="header" text="Kroniek" heading-level="3" collapse-anchor="kroniek-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="kroniek-titel" size="display-3"><h3>Kroniek</h3></nldd-title>
          <nldd-text color="secondary">Alles wat een cel vastlegde, in de volgorde waarin het gebeurde.</nldd-text>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-activity-indicator v-if="laden" text="De kronieken laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De kroniek laadt niet" :supporting-text="fout"></nldd-inline-dialog>
          <nldd-inline-dialog v-else-if="kronieken.length === 0" icon="info" text="U mag geen kroniek lezen"></nldd-inline-dialog>
          <nldd-list v-else appearance="simple" accessible-label="Cellen">
            <nldd-list-item
              v-for="k in kronieken"
              :key="k.cel"
              size="md"
              button
              :current="k.cel === gekozen?.cel || undefined"
              @click="kies(k.cel)"
            >
              <nldd-icon-cell size="20"><nldd-icon icon="history"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell :text="hoofdletter(k.cel_naam)" :supporting-text="`${k.grammen.length} vastgelegd`"></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" :has-content="gekozen ? true : undefined">
      <nldd-page v-if="gekozen && casus" landmarks="page" sticky-header :accessible-label="`Kroniek van ${gekozen.cel_naam}`">
        <nldd-top-title-bar
          slot="header"
          :text="hoofdletter(gekozen.cel_naam)"
          back-text="Kroniek"
          collapse-anchor="cel-kroniek-titel"
        ></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="cel-kroniek-titel" size="display-3"><h3>{{ hoofdletter(gekozen.cel_naam) }}</h3></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-list v-if="gekozen.grammen.length" appearance="box-base" :accessible-label="`Kroniek van ${gekozen.cel_naam}`">
            <nldd-list-item v-for="(g, i) in gekozen.grammen" :key="g.id" size="sm">
              <nldd-text-cell
                :overline="`${i + 1}`"
                :text="omschrijving(g, casus)"
                :supporting-text="`${longDate(g.recorded_on)} · ${g.basis}${g.refers_to ? ` · verwijst naar ${g.refers_to}` : ''}`"
              ></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-inline-dialog v-else icon="info" text="De kroniek is leeg"></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
      <nldd-page v-else landmarks="page">
        <nldd-simple-section width="full">
          <nldd-inline-dialog text="Kies een cel" supporting-text="De kroniek van een cel opent hier."></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>
