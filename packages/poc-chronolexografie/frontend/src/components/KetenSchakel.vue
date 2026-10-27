<script setup>
// One step of the chain of a case (tab Keten), as a row of an nldd-list of
// type tree, like the steps in the Logboek (KetenStap.vue). A row with steps
// under it is a branch that opens; the depth is indented with a spacer cell
// per level. An article opens onto its explanation, its layer and file, a
// button for its YAML and a link to its source, and then the steps it leads
// to. The YAML opens in a sheet (KetenTab.vue), as the Kroniek shows JSON.
import { computed, ref } from 'vue';
import { state } from '../state.js';
import { bestandPad, heeftKinderen, korteUitleg, laagNaam, schakelTekst } from '../keten.js';

defineOptions({ name: 'KetenSchakel' });

const props = defineProps({
  // A step from `api.keten` (src/keten.rs, `Schakel`).
  schakel: { type: Object, required: true },
  diepte: { type: Number, default: 0 },
  // Whether the row starts open: the root does, so the first level shows.
  open: { type: Boolean, default: false },
});
const emit = defineEmits(['yaml']);

const isOpen = ref(props.open);
const s = computed(() => props.schakel);
const tekst = computed(() => schakelTekst(s.value));
const artikel = computed(() => s.value.soort === 'artikel');
// An article always opens, onto its details; another step only onto steps under it.
const tak = computed(() => artikel.value || heeftKinderen(s.value) || (s.value.soort === 'begrip' && !!s.value.beschrijving));
// The explanation gets a row of its own unless the row itself already says it.
const uitleg = computed(() => {
  const u = artikel.value ? korteUitleg(s.value.tekst) : (s.value.beschrijving ?? '');
  return u === tekst.value.toelichting ? '' : u;
});
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
      <nldd-list-item v-if="uitleg" slot="children" size="sm" @click.stop>
        <nldd-spacer-cell v-for="n in diepte + 1" :key="n" size="24"></nldd-spacer-cell>
        <nldd-spacer-cell size="24"></nldd-spacer-cell>
        <nldd-text-cell color="secondary" :text="uitleg"></nldd-text-cell>
      </nldd-list-item>
      <template v-if="artikel">
        <nldd-list-item slot="children" size="sm" @click.stop>
          <nldd-spacer-cell v-for="n in diepte + 1" :key="n" size="24"></nldd-spacer-cell>
          <nldd-spacer-cell size="24"></nldd-spacer-cell>
          <nldd-text-cell
            color="secondary"
            :text="`${laagNaam(s.laag)}${s.naam ? ` · ${s.naam}` : ''}`"
            :supporting-text="pad"
          ></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item slot="children" size="sm" @click.stop>
          <nldd-spacer-cell v-for="n in diepte + 1" :key="n" size="24"></nldd-spacer-cell>
          <nldd-spacer-cell size="24"></nldd-spacer-cell>
          <nldd-list-item-segment v-if="s.yaml" button @click.stop="emit('yaml', s)">
            <nldd-text-cell color="accent" text="YAML bekijken"></nldd-text-cell>
          </nldd-list-item-segment>
          <nldd-list-item-segment v-if="s.url" :href="s.url" target="_blank" @click.stop>
            <nldd-text-cell color="accent" text="Bron"></nldd-text-cell>
          </nldd-list-item-segment>
        </nldd-list-item>
      </template>
      <KetenSchakel
        v-for="(k, i) in s.kinderen ?? []"
        :key="i"
        slot="children"
        :schakel="k"
        :diepte="diepte + 1"
        @yaml="emit('yaml', $event)"
      />
    </template>
  </nldd-list-item>
</template>
