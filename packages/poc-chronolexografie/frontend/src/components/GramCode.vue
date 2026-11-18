<script setup>
// One chronolexogram of a chronicle: its fact and main fields in view, what
// it refers to and what refers to it as links to the Kroniek tab
// (verwijzingen.js), the gram itself as highlighted JSON behind a button
// (UitklapCode.vue). The Kroniek tab and the sheet with the grams a
// reduction read both show a gram this way.
import { computed } from 'vue';
import { alsJson, hoofdvelden } from '../state.js';
import { gramKop, verwezenDoor, verwijzingen } from '../verwijzingen.js';
import GramLink from './GramLink.vue';
import UitklapCode from './UitklapCode.vue';

const props = defineProps({
  gram: { type: Object, required: true },
  // Its place in its chronicle, from 1, when the list around it counts.
  nummer: { type: Number, default: null },
  // The title's level in the outline around it (see UitklapCode.vue).
  headingLevel: { type: Number, default: 3 },
  // Whether a link just led here (KroniekTab.vue).
  gemarkeerd: { type: Boolean, default: false },
});

const labels = computed(() => hoofdvelden(props.gram));
const naam = computed(() => gramKop(props.gram));
const kop = computed(() => props.gram.fact ?? naam.value);
const boven = computed(() =>
  [props.nummer, props.gram.recorded_on, props.gram.fact && naam.value].filter(Boolean).join(' · '),
);
const naar = computed(() => verwijzingen(props.gram));
const door = computed(() => verwezenDoor(props.gram));
</script>

<template>
  <UitklapCode
    :overline="boven"
    :text="kop"
    :supporting-text="gram.basis"
    :naam="naam"
    language="json"
    :code="alsJson(gram)"
    :heading-level="headingLevel"
  >
    <nldd-container v-if="labels.length || gemarkeerd" layout="wrap" gap="4">
      <nldd-badge v-if="gemarkeerd" size="sm" color="accent" text="Hierheen verwezen"></nldd-badge>
      <nldd-tag v-for="w in labels" :key="w" size="sm" :text="w"></nldd-tag>
    </nldd-container>
    <nldd-text v-for="r in naar" :key="r.id" size="sm">
      {{ r.soort }}: <GramLink :doel="r.doel" />
    </nldd-text>
    <nldd-text v-if="door.length" size="sm">
      Verwezen door:
      <template v-for="(d, i) in door" :key="d.id"
        ><GramLink :doel="d" /><template v-if="i < door.length - 1">, </template></template
      >
    </nldd-text>
  </UitklapCode>
</template>
