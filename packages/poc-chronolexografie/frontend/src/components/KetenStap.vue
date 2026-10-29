<script setup>
// Eén stap uit de keten van een besluit, als rij van een nldd-list van type
// tree. Een stap met stappen eronder is een tak die openklapt; de diepte
// staat als inspringing (een spacer-cell per niveau), zoals de lijst dat
// vraagt.
import { computed, ref } from 'vue';
import { state } from '../state.js';
import { stapTekst } from '../logboek.js';

defineOptions({ name: 'KetenStap' });

const props = defineProps({
  // Een stap uit `genummerd()`: { nummer, stap, kinderen }.
  knoop: { type: Object, required: true },
  diepte: { type: Number, default: 0 },
});

const open = ref(true);
const tak = computed(() => props.knoop.kinderen.length > 0);
const tekst = computed(() => stapTekst(props.knoop.stap, state.grammen));
</script>

<template>
  <nldd-list-item
    size="sm"
    :button="tak || undefined"
    :expanded="tak ? open : undefined"
    @click.stop="tak && (open = !open)"
  >
    <nldd-spacer-cell v-for="n in diepte" :key="n" size="24"></nldd-spacer-cell>
    <nldd-icon-cell v-if="tak" disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
    <nldd-spacer-cell v-else size="16"></nldd-spacer-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-text-cell width="fit-content" color="secondary" :text="knoop.nummer"></nldd-text-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-text-cell :text="tekst.tekst" :supporting-text="tekst.toelichting"></nldd-text-cell>
    <KetenStap
      v-for="k in knoop.kinderen"
      :key="k.nummer"
      slot="children"
      :knoop="k"
      :diepte="diepte + 1"
    />
  </nldd-list-item>
</template>
