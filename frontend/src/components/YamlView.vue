<script setup lang="ts">
import { computed, inject } from 'vue';
import * as yaml from 'js-yaml';
import MachineEmptyState from './MachineEmptyState.vue';
import type { LawArticle } from '../lib/lawFormat';

const props = withDefaults(
  defineProps<{
    article?: LawArticle | null;
    /** Read-only context: offer a button that opens the editor so the missing
     *  machine-readable version can be created there. */
    canCreate?: boolean;
    /** Whether an enrichment can be requested here. The run covers the whole
     *  law, not just this article, so the empty state says so. */
    canEnrich?: boolean;
    /** Doorgegeven aan MachineEmptyState: '' | 'login' | 'traject'. */
    needs?: string;
    /** An enrichment for this law is already pending or processing. The empty
     *  state then reports that instead of offering the buttons again. */
    enriching?: boolean;
    /** Message from the most recent failed enrich request, shown in place of
     *  the supporting text. */
    enrichError?: string;
    /** Er staat een openstaande review-taak klaar voor deze wet. */
    reviewReady?: boolean;
    reviewArticle?: string;
    /** Anchor target for that button. Leave unset when the user isn't logged
     *  in, so the click gates on the login popover instead of the href. */
    createHref?: string;
  }>(),
  {
    article: null,
    canCreate: false,
    canEnrich: false,
    needs: '',
    enriching: false,
    enrichError: '',
    reviewReady: false,
    reviewArticle: '',
    createHref: undefined,
  },
);

const emit = defineEmits<{
  /** Create-button click. Payload is the button element, so the parent can
   *  anchor the login popover to it. */
  'create-mr': [anchor: EventTarget | null];
  /** Ask for a generated proposal instead of writing one by hand. */
  enrich: [anchor: EventTarget | null, provider?: string];
  /** Open the task list filtered to this law, while a run is pending. */
  'view-tasks': [];
  /** Open review-modus voor de klaarstaande taak. */
  review: [];
}>();

// Provided by AppShell; see LibraryView's "Bewerken" button.
const onLoginTriggerPointerdown = inject('onLoginTriggerPointerdown', () => {});

const yamlText = computed(() => {
  const mr = props.article?.machine_readable;
  if (!mr) return null;
  return yaml.dump(mr, { lineWidth: 80, noRefs: true });
});
</script>

<template>
  <!-- Zelfde ingang als in de Machine-pane: die twee staan niet altijd allebei
       open. De supporting-text benoemt de scope, want de knop staat in een
       weergave die over één artikel gaat. -->
  <MachineEmptyState
    v-if="!yamlText"
    :review-ready="reviewReady"
    :review-article="reviewArticle"
    :enriching="enriching"
    :can-enrich="canEnrich"
    :needs="needs"
    :can-create="canCreate"
    :create-href="createHref"
    :enrich-error="enrichError"
    @enrich="(trigger: EventTarget | null, provider?: string) => emit('enrich', trigger, provider)"
    @create="emit('create-mr', $event)"
    @view-tasks="emit('view-tasks')"
    @review="emit('review')"
  ></MachineEmptyState>
  <nldd-code-viewer v-else language="yaml">{{ yamlText }}</nldd-code-viewer>
</template>
