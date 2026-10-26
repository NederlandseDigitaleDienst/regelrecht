<script setup>
// Eén stap uit de keten van een besluit, als rij van een nldd-list van type
// tree. Een stap met stappen eronder is een tak die openklapt; de diepte
// staat als inspringing (een spacer-cell per niveau), zoals de lijst dat
// vraagt. Bij een begrip dat de cel beantwoordde staat een knop die haar
// eigen uitleg opent.
import { computed, ref } from 'vue';
import { state } from '../state.js';
import { eigenAntwoord, stapTekst } from '../logboek.js';

defineOptions({ name: 'KetenStap' });

const props = defineProps({
  // Een stap uit `genummerd()`: { nummer, stap, kinderen }. Bij een
  // antwoord van de cel staat in `stap.keer` de hoeveelste keer het besluit
  // haar dit begrip vroeg.
  knoop: { type: Object, required: true },
  diepte: { type: Number, default: 0 },
  // Of de knop naar de uitleg van de cel er staat. Bij een voorvertoning
  // niet: die uitleg leest een vastgelegd besluit, en dat is er nog niet.
  metUitleg: { type: Boolean, default: true },
});
const emit = defineEmits(['uitleg']);

const open = ref(true);
const tak = computed(() => props.knoop.kinderen.length > 0);
const tekst = computed(() => stapTekst(props.knoop.stap, state.grammen));
const cel = computed(() =>
  props.metUitleg && props.knoop.stap.keer != null ? eigenAntwoord(props.knoop.stap) : null,
);
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
    <nldd-list-item-segment v-if="cel" button @click.stop="emit('uitleg', knoop)">
      <nldd-text-cell color="accent" text="Hoe de cel antwoordde"></nldd-text-cell>
    </nldd-list-item-segment>
    <KetenStap
      v-for="k in knoop.kinderen"
      :key="k.nummer"
      slot="children"
      :knoop="k"
      :diepte="diepte + 1"
      :met-uitleg="metUitleg"
      @uitleg="emit('uitleg', $event)"
    />
  </nldd-list-item>
</template>
