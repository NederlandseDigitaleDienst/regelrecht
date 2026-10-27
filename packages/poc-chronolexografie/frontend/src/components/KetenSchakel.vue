<script setup>
// One step of the chain of a case (tab Keten), as a row of an nldd-list of
// type tree, like the steps in the Logboek (KetenStap.vue). A row with steps
// under it is a branch that opens; the depth is indented with a spacer cell
// per level. Every row starts open. A step shows a short heading, an
// explanation of two or three sentences (keten.js), and one panel per block
// of YAML from the file that makes this link of the chain (src/fragment.rs).
// An article then has a button for the full article, a link to its source
// and its file, and the steps it leads to. The full article opens in a sheet
// (KetenTab.vue), as the Kroniek shows JSON.
import { computed, ref } from 'vue';
import { state } from '../state.js';
import { bestandPad, heeftKinderen, laagNaam, schakelSleutel, schakelTekst } from '../keten.js';

defineOptions({ name: 'KetenSchakel' });

const props = defineProps({
  // A step from `api.keten` (src/keten.rs, `Schakel`).
  schakel: { type: Object, required: true },
  diepte: { type: Number, default: 0 },
  // Whether the row starts open, so each fragment shows without a click.
  open: { type: Boolean, default: true },
});
const emit = defineEmits(['yaml']);

const isOpen = ref(props.open);
const s = computed(() => props.schakel);
const tekst = computed(() => schakelTekst(s.value, { wortel: props.diepte === 0 }));
const artikel = computed(() => s.value.soort === 'artikel');
// Every step opens onto its explanation; it has one of every kind.
const tak = computed(() => !!tekst.value.uitleg || heeftKinderen(s.value));
const panelen = computed(() => s.value.fragmenten ?? []);
const pad = computed(() => bestandPad(s.value.bestand, state.casus?.bron));
</script>

<template>
  <nldd-list-item
    size="sm"
    :button="tak || undefined"
    :expanded="tak ? isOpen : undefined"
    @click.stop="tak && (isOpen = !isOpen)"
  >
    <nldd-spacer-cell v-for="n in diepte" :key="n" size="24"></nldd-spacer-cell>
    <nldd-icon-cell v-if="tak" disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
    <nldd-spacer-cell v-else size="16"></nldd-spacer-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-text-cell :overline="tekst.overline" :text="tekst.tekst" :supporting-text="tekst.toelichting"></nldd-text-cell>
    <nldd-cell v-if="s.fictief">
      <nldd-badge size="sm" color="warning" text="fictief"></nldd-badge>
    </nldd-cell>

    <template v-if="tak">
      <nldd-list-item v-if="tekst.uitleg" slot="children" size="sm" @click.stop>
        <nldd-spacer-cell v-for="n in diepte + 1" :key="n" size="24"></nldd-spacer-cell>
        <nldd-spacer-cell size="24"></nldd-spacer-cell>
        <nldd-text-cell :text="tekst.uitleg"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item v-for="(p, i) in panelen" :key="`paneel-${i}`" slot="children" size="sm" @click.stop>
        <nldd-spacer-cell v-for="n in diepte + 1" :key="n" size="24"></nldd-spacer-cell>
        <nldd-spacer-cell size="24"></nldd-spacer-cell>
        <nldd-cell width="full">
          <nldd-code-viewer language="yaml" appearance="box-tinted" max-height="320px">{{ p }}</nldd-code-viewer>
        </nldd-cell>
      </nldd-list-item>
      <nldd-list-item v-if="artikel" slot="children" size="sm" @click.stop>
        <nldd-spacer-cell v-for="n in diepte + 1" :key="n" size="24"></nldd-spacer-cell>
        <nldd-spacer-cell size="24"></nldd-spacer-cell>
        <nldd-list-item-segment v-if="s.yaml" button @click.stop="emit('yaml', s)">
          <nldd-text-cell color="accent" text="Volledig artikel"></nldd-text-cell>
        </nldd-list-item-segment>
        <nldd-list-item-segment v-if="s.url" :href="s.url" target="_blank" @click.stop>
          <nldd-text-cell color="accent" text="Bron"></nldd-text-cell>
        </nldd-list-item-segment>
        <nldd-text-cell
          color="secondary"
          :text="`${laagNaam(s.laag)}${s.naam ? ` · ${s.naam}` : ''}`"
          :supporting-text="pad"
        ></nldd-text-cell>
      </nldd-list-item>
      <KetenSchakel
        v-for="(k, i) in s.kinderen ?? []"
        :key="schakelSleutel(k, i)"
        slot="children"
        :schakel="k"
        :diepte="diepte + 1"
        @yaml="emit('yaml', $event)"
      />
    </template>
  </nldd-list-item>
</template>
