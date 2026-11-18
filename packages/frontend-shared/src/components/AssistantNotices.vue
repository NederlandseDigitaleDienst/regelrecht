<template>
  <div v-if="visible.length" class="assistant-notices">
    <nldd-banner
      v-for="n in visible"
      :key="n.id"
      :variant="n.kind === 'question' ? 'warning' : 'accent'"
      dismissible
      @dismiss="$emit('dismiss', n.id)"
    >
      {{ n.text }}
      <nldd-button
        v-if="goToLabel"
        slot="actions"
        size="sm"
        appearance="secondary"
        :text="goToLabel"
        @click="$emit('go')"
      ></nldd-button>
    </nldd-banner>
  </div>
</template>

<!--
  Notices from the policy assistant, on every page.

  Why in the app and not only as a browser notification: a notification only
  makes sense when the page is hidden. Someone who walked over to Scenario's
  within the app still has the page open, and so sees nothing. That is
  exactly the case this started from: working on in another tab while the
  assistant computes.

  The notices live here, apart from the assistant panel, because that panel is
  not mounted on those other pages.
-->

<script setup lang="ts">
import { computed } from 'vue';

/**
 * A notice from the assistant. A `question` waits for the user; any other
 * kind (such as `done`) only informs.
 */
export interface Notice {
  id: number;
  kind: 'question' | 'done' | (string & {});
  text: string;
}

const props = withDefaults(
  defineProps<{
    notices?: Notice[];
    /** Is the user already on the assistant's page? Then this is not needed. */
    onAssistantPage?: boolean;
    /** Text of the button that goes to the assistant; empty leaves it out. */
    goToLabel?: string;
  }>(),
  { notices: () => [], onAssistantPage: false, goToLabel: '' },
);
defineEmits<{ dismiss: [id: number]; go: [] }>();

/**
 * On the assistant's own page a banner says nothing: the conversation is
 * already in view. At most the last two, because a row of banners pushes the
 * content away.
 */
const visible = computed(() => (props.onAssistantPage ? [] : props.notices.slice(-2)));
</script>

<style scoped>
.assistant-notices {
  display: flex; flex-direction: column; gap: var(--primitives-space-8);
  padding: var(--primitives-space-8) var(--primitives-space-24);
}
</style>
