<script setup lang="ts">
// "Alle regelwerken": every law in Corpus juris, a page at a time, with the
// "only enriched" filter it shares with the search popover. Lives in the
// secondary sidebar, the way Werkdocumenten does in a traject; picking a law
// opens it like any other.
import { ref, watch } from 'vue';
import { useLawOverview, enrichedOnly, pageCount } from '../composables/useLawOverview';
import type { CorpusLaw } from '../composables/useCorpusLaws';
import { humanizeLawId } from '../lib/lawName';
import PaginationControls from '../harvester/components/PaginationControls.vue';

defineProps<{ backText: string }>();
const emit = defineEmits<{ select: [lawId: string] }>();

const page = ref(1);
const { laws, total, loading, error, reload } = useLawOverview(page);

// A different filter is a different list: start it at its first page.
watch(enrichedOnly, () => {
  page.value = 1;
});

function onFilterChange(event: Event) {
  enrichedOnly.value = !!(event as CustomEvent<{ checked?: boolean } | undefined>).detail?.checked;
}

function displayName(law: CorpusLaw): string {
  return law.display_name || law.name || humanizeLawId(law.law_id);
}

// The source, and whether the version shown has a machine version: the one
// thing this list says that the law's name does not.
function supportingText(law: CorpusLaw): string | undefined {
  const parts = [law.source_name, law.enriched ? 'machineversie' : null].filter(Boolean);
  return parts.length ? parts.join(' · ') : undefined;
}
</script>

<template>
  <nldd-page sticky-header>
    <nldd-top-title-bar slot="header" text="Alle regelwerken" :back-text="backText" collapse-anchor="overzicht-titel"></nldd-top-title-bar>
    <nldd-simple-section width="full">
      <nldd-title id="overzicht-titel" size="3"><h3>Alle regelwerken</h3></nldd-title>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-checkbox-field
        data-testid="overview-enriched-filter"
        label="Alleen verrijkte regelwerken"
        :checked="enrichedOnly || undefined"
        @change="onFilterChange"
      ></nldd-checkbox-field>
      <nldd-spacer size="16"></nldd-spacer>

      <nldd-activity-indicator v-if="loading && !laws.length" text="Regelwerken laden" show-text></nldd-activity-indicator>
      <nldd-inline-dialog
        v-else-if="error"
        variant="alert"
        text="De regelwerken zijn niet geladen"
        :supporting-text="error"
      >
        <nldd-button slot="actions" appearance="secondary" text="Probeer opnieuw" @click="reload"></nldd-button>
      </nldd-inline-dialog>
      <nldd-inline-dialog
        v-else-if="!laws.length"
        :text="enrichedOnly ? 'Nog geen verrijkte regelwerken' : 'Geen regelwerken'"
        :supporting-text="enrichedOnly ? 'Een verrijkt regelwerk heeft naast de wettekst ook een machineversie.' : undefined"
      ></nldd-inline-dialog>
      <template v-else>
        <nldd-list appearance="simple" data-testid="overview-list">
          <nldd-list-item
            v-for="law in laws"
            :key="law.law_id"
            size="md"
            button
            :data-law-id="law.law_id"
            @click="emit('select', law.law_id)"
          >
            <nldd-text-cell :text="displayName(law)" :supporting-text="supportingText(law)"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-spacer size="16"></nldd-spacer>
        <PaginationControls :current-page="page" :total-pages="pageCount(total)" @page-change="page = $event" />
      </template>
    </nldd-simple-section>
  </nldd-page>
</template>
