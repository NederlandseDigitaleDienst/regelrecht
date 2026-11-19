<script setup lang="ts">
// The chronicle of a cell the employee may read: every record, in the order
// it was recorded. Nothing in it changes or goes away.
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

const gekozen = computed(() => kronieken.value.find((k) => k.cel === props.cel) ?? kronieken.value[0]);
const kies = (e: Event) => router.push({ name: 'kroniek', params: { cel: (e.target as HTMLSelectElement).value } });
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" heading-level="1" text="Kroniek" supporting-text="Alles wat een cel vastlegde, in de volgorde waarin het gebeurde."></nldd-title>
    <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De kroniek laadt niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-inline-dialog v-else-if="!gekozen" icon="info" text="U mag geen kroniek lezen"></nldd-inline-dialog>
    <nldd-container v-else-if="casus" gap="16">
      <nldd-form-field v-if="kronieken.length > 1" label="Cel">
        <nldd-dropdown>
          <select :value="gekozen.cel" @change="kies">
            <option v-for="k in kronieken" :key="k.cel" :value="k.cel">{{ hoofdletter(k.cel_naam) }}</option>
          </select>
        </nldd-dropdown>
      </nldd-form-field>
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
    </nldd-container>
  </nldd-simple-section>
</template>
