<script setup lang="ts">
/**
 * HotspotsSidebarBlock - "Kan verrijking gebruiken" in the library's primary
 * sidebar: the five articles of the traject where enrichment would help most,
 * each with the first reason of its score and how many articles elsewhere
 * depend on it. A row opens that article with the enrich sheet at its first
 * step, the chain, so the engineer sees what it touches before choosing.
 *
 * Collapsible the way the title of a section does it elsewhere in this
 * sidebar: an xs button at the end of the title. Hidden when there is
 * nothing to show; a traject without gaps needs no block that says so.
 */
import { computed, ref } from 'vue';
import { useHotspots } from '../composables/useHotspots';
import { refText, topReason } from '../lib/enrichChain';
import { useCorpus } from '../corpus';

const props = withDefaults(
  defineProps<{
    trajectRef?: string | null;
    /** Flag `enrich.hotspots`. */
    enabled?: boolean;
  }>(),
  { trajectRef: null, enabled: false },
);

const emit = defineEmits<{
  open: [lawId: string, article: string];
}>();

const { displayName } = useCorpus();
const { hotspots } = useHotspots({
  trajectRef: () => props.trajectRef,
  enabled: () => props.enabled,
  limit: 5,
});

const expanded = ref(true);
const rows = computed(() => hotspots.value.hotspots);
</script>

<template>
  <template v-if="rows.length">
    <nldd-title size="display-5">
      <h4>Kan verrijking gebruiken</h4>
      <nldd-button
        slot="end"
        size="xs"
        appearance="accent-transparent"
        data-testid="hotspots-toggle"
        :text="expanded ? 'Verberg' : 'Toon'"
        @click="expanded = !expanded"
      ></nldd-button>
    </nldd-title>
    <template v-if="expanded">
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list appearance="simple" accessible-label="Artikelen die verrijking kunnen gebruiken">
        <nldd-list-item
          v-for="h in rows"
          :key="`${h.law_id}#${h.article}`"
          size="md"
          button
          :data-hotspot="`${h.law_id}#${h.article}`"
          @click="emit('open', h.law_id, h.article)"
        >
          <nldd-text-cell
            :text="refText(`${h.law_id}#${h.article}`, displayName)"
            :supporting-text="topReason(h.reasons, displayName)"
          ></nldd-text-cell>
          <template v-if="h.dependants > 0">
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-cell>
              <nldd-badge
                size="sm"
                color="neutral"
                :number="h.dependants"
                :accessible-label="`${h.dependants} artikelen hangen hiervan af`"
              ></nldd-badge>
            </nldd-cell>
            <nldd-spacer-cell size="2"></nldd-spacer-cell>
          </template>
          <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-text-cell
        v-if="hotspots.partial"
        supporting-text="Nog niet alle wetten zijn gelezen; de lijst kan nog veranderen."
      ></nldd-text-cell>
    </template>
    <nldd-spacer size="24"></nldd-spacer>
  </template>
</template>
