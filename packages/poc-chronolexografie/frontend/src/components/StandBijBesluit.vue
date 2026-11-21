<script setup>
// Beside a decision of the case's cell: what the cell knows about it on the
// play date, per view the UI spec of the case puts there
// (`state.vormen.per_besluit`, stand.js), such as what is still to be paid
// on it and from when the authority is in default. A field the view asks is
// what the authority gives besides; left empty, the regelwerk says what
// follows. Nothing is recorded.
import { reactive, ref, watch } from 'vue';
import { hoofdletter, leesbaar, longDate, state } from '../state.js';
import { label, toonWaarde, vraagStand } from '../stand.js';
import VeldInvoer from '@regelrecht/frontend-shared/components/chronolex/VeldInvoer.vue';

const props = defineProps({
  // The decision, a chronolexogram of the case's cell.
  besluit: { type: Object, required: true },
  // The views of `state.vormen.per_besluit`.
  weergaven: { type: Array, required: true },
});

// Per view what the authority gives in the fields it asks, set up once per
// view so that reading it during render writes nothing.
const gegeven = reactive({});
watch(
  () => props.weergaven,
  (weergaven) => weergaven.forEach((w) => (gegeven[w.naam] ??= {})),
  { immediate: true },
);
const waarden = (naam) => gegeven[naam] ?? {};

// Per view the answers, asked again whenever the play date, a chronicle or
// a given value changes; a later answer wins over one still on its way.
const stand = ref({});
let vraag = 0;
watch(
  [
    () => state.datum,
    () => `${state.grammen.length}:${state.grammen.at(-1)?.id}`,
    () => JSON.stringify(gegeven),
    () => props.weergaven,
  ],
  async () => {
    const deze = ++vraag;
    const uit = {};
    for (const w of props.weergaven) {
      uit[w.naam] = await vraagStand(w, props.besluit.id, state.datum, waarden(w.naam));
    }
    if (deze === vraag) stand.value = uit;
  },
  { immediate: true },
);

const tekst = (a) => (a.fout ? `geen antwoord (${a.fout})` : toonWaarde(a.kreeg, a));
</script>

<template>
  <nldd-container v-for="w in weergaven" :key="w.naam" gap="8">
    <nldd-title heading-level="4" size="display-6" :text="hoofdletter(leesbaar(w.naam))"></nldd-title>
    <template v-for="v in w.velden" :key="v.naam">
      <nldd-inline-dialog
        v-if="v.fout"
        icon="warning"
        icon-color="warning"
        :text="leesbaar(v.naam)"
        :supporting-text="v.fout"
      ></nldd-inline-dialog>
      <VeldInvoer
        v-else
        :veld="v"
        :model-value="waarden(w.naam)[v.naam]"
        @update:model-value="gegeven[w.naam][v.naam] = $event"
      />
    </template>
    <nldd-list appearance="box-base" :accessible-label="`${leesbaar(w.naam)} op ${longDate(state.datum)}`">
      <nldd-list-item v-for="a in stand[w.naam] ?? []" :key="a.veld" size="sm">
        <nldd-text-cell :text="label(a)" :supporting-text="a.beschrijving ?? ''"></nldd-text-cell>
        <nldd-text-cell width="fit-content" :text="tekst(a)"></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>
  </nldd-container>
</template>
