<script setup lang="ts">
import { computed } from 'vue';
import { useRouter } from 'vue-router';
import { RE_HARVESTABLE_STATUSES, ENRICHABLE_STATUSES } from '../constants';
import { apiFetch } from '@regelrecht/frontend-shared';
import type { LawEntry } from '../composables/useLawEntries';
import { providerLabel } from '../../lib/enrichProviders';

const props = defineProps<{
  row: LawEntry;
}>();

const emit = defineEmits<{
  'action-complete': [];
}>();

/** The response of POST /harvest-jobs (`CreateJobResponse` in packages/admin/src/handlers.rs). */
interface HarvestJobCreated {
  job_id: string;
}

/** The response of POST /enrich-jobs (`CreateEnrichResponse` in packages/admin/src/handlers.rs). */
interface EnrichJobsCreated {
  job_ids: string[];
  providers: string[];
}

const router = useRouter();

function onViewJobs() {
  router.push({ name: 'jobs', query: { law_id: props.row.law_id } });
}

const canHarvest = computed(() => RE_HARVESTABLE_STATUSES.includes(props.row.status));
const canEnrich = computed(() => ENRICHABLE_STATUSES.includes(props.row.status));
const canReset = computed(
  () => props.row.status === 'harvest_exhausted' || props.row.status === 'enrich_exhausted',
);

async function onHarvest() {
  try {
    const response = await apiFetch('/api/harvest-admin/harvest-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bwb_id: props.row.law_id }),
      allowStatuses: [401, 409],
    });
    // 401 is handled by the editor's global apiAuthGuard (redirect to login).
    if (response.status === 401) return;
    if (response.status === 409) {
      const text = await response.text().catch(() => '');
      alert(text || 'A harvest job for this law is already pending or processing.');
      return;
    }
    const result = (await response.json()) as HarvestJobCreated;
    alert(`Created harvest job: ${result.job_id}`);
    emit('action-complete');
  } catch (err) {
    alert('Harvest failed: ' + (err as Error).message);
  }
}

async function onEnrich() {
  try {
    const response = await apiFetch('/api/harvest-admin/enrich-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ law_id: props.row.law_id }),
      allowStatuses: [401, 409],
    });
    if (response.status === 401) return;
    if (response.status === 409) {
      const text = await response.text().catch(() => '');
      alert(text || 'Enrich jobs for this law are already pending or processing.');
      return;
    }
    const result = (await response.json()) as EnrichJobsCreated;
    alert(`Created ${result.job_ids.length} enrich job(s) for ${result.providers.map(providerLabel).join(', ')}`);
    emit('action-complete');
  } catch (err) {
    alert('Enrich failed: ' + (err as Error).message);
  }
}

async function onResetExhausted() {
  try {
    const response = await apiFetch(
      `/api/harvest-admin/law_entries/${encodeURIComponent(props.row.law_id)}/reset-exhausted`,
      { method: 'POST', allowStatuses: [401] },
    );
    if (response.status === 401) return;
    emit('action-complete');
  } catch (err) {
    alert('Reset failed: ' + (err as Error).message);
  }
}
</script>

<template>
  <!-- The menu sits in the button's popup slot: the button anchors and toggles
       it itself, so no per-instance id is needed. -->
  <nldd-icon-button
    icon="ellipsis"
    text="Actions"
    tooltip-timing="never"
    appearance="neutral-tinted"
    size="md"
  >
    <nldd-menu slot="popup">
      <nldd-menu-item v-if="canHarvest" text="Harvest" @click.stop="onHarvest" />
      <nldd-menu-item v-if="canEnrich" text="Enrich" @click.stop="onEnrich" />
      <nldd-menu-item v-if="canReset" text="Reset exhausted" @click.stop="onResetExhausted" />
      <nldd-menu-item text="View job details" @click.stop="onViewJobs" />
    </nldd-menu>
  </nldd-icon-button>
</template>
