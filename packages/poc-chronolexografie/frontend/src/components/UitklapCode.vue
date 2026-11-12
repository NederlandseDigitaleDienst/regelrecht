<script setup>
// A title with a "Bekijk" button that folds open a highlighted block of code
// below it, closed at first: a chronolexogram as JSON (GramCode.vue), the
// `machine_readable` of an article as YAML (HerkomstSheet.vue). What sits
// between the title and the code goes in the slot.
import { ref } from 'vue';

defineProps({
  text: { type: String, required: true },
  overline: { type: String, default: undefined },
  supportingText: { type: String, default: undefined },
  // What the button shows, for a screen reader: "Bekijk <naam>".
  naam: { type: String, required: true },
  language: { type: String, required: true },
  code: { type: String, required: true },
  // The title's level in the outline around it; 3 under a section's h2.
  headingLevel: { type: Number, default: 3 },
});

const open = ref(false);
</script>

<template>
  <nldd-container gap="8">
    <nldd-title :heading-level="headingLevel" size="display-6" :overline="overline" :text="text" :supporting-text="supportingText">
      <nldd-button
        slot="end"
        appearance="neutral-tinted"
        size="sm"
        text="Bekijk"
        expandable
        :expanded="open"
        :accessible-label="`Bekijk ${naam}`"
        @click="open = !open"
      ></nldd-button>
    </nldd-title>
    <slot />
    <nldd-code-viewer v-if="open" :language="language" appearance="box-tinted">{{ code }}</nldd-code-viewer>
  </nldd-container>
</template>
