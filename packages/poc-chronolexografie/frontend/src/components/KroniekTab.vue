<script setup>
// The chronicle of the cell: every chronolexogram, in the order it was
// recorded (RFC-046), with its fact and values in view (GramCode.vue).
// Below it the chronicles of the other cells of the case (RFC-048): each
// cell keeps its own; the bestuursorgaan asks them, but does not write in
// them. The court is one of them (RFC-053).
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { state } from '../state.js';
import { inBeeld } from '../verwijzingen.js';
import GramCode from './GramCode.vue';

// The chronicle of the cell of the case, then those of the other cells.
const kronieken = computed(() => [
  { naam: state.casus.cel_naam, grammen: state.grammen, eigen: true, rechter: false },
  ...state.andere.map((a) => ({
    naam: a.cel_naam,
    grammen: a.grammen,
    eigen: false,
    rechter: a.cel === state.casus.rechter?.cel,
  })),
]);
const bestaat = (id) => kronieken.value.some((k) => k.grammen.some((g) => g.id === id));

// From the log or a reference (verwijzingen.js): bring that chronolexogram
// into view, once it is there, give it focus and mark it.
const gemarkeerd = ref(null);
watch(
  [() => state.inKroniek, kronieken],
  async ([id]) => {
    if (!id || !bestaat(id)) return;
    state.inKroniek = null;
    gemarkeerd.value = id;
    await inBeeld(`gram-${id}`);
  },
  { immediate: true },
);
// A request this tab could not find does not wait for the next visit.
onBeforeUnmount(() => {
  state.inKroniek = null;
});
</script>

<template>
  <nldd-simple-section v-for="k in kronieken" :key="k.naam">
    <nldd-title heading-level="2" slot="header" size="display-3" :text="`Kroniek van ${k.naam}`">
      <nldd-text v-if="k.eigen" slot="supporting-text">
        <p>
          Eén kroniek, van de cel van de uitvoerder. Elke stap in het proces legt er één
          chronolexogram in vast (een beslissing op bezwaar die herroept, legt er twee vast); er wordt
          nooit iets in gewijzigd of uit verwijderd.<template v-if="state.casus?.begin"> Bovenaan
          staat wat de cel vóór het eerste scenario vastlegde, zoals de vestiging van een school
          (RFC-051).</template>
        </p>
      </nldd-text>
      <nldd-text v-else-if="k.rechter" slot="supporting-text">
        <p>
          De kroniek van de rechtbank, een andere cel. Een beroep verwijst niet naar het bestreden
          besluit, want dat staat in een andere kroniek: het draagt wat de rechtbank ontving.
        </p>
      </nldd-text>
      <nldd-text v-else slot="supporting-text"><p>Een andere cel van de casus, met haar eigen kroniek.</p></nldd-text>
    </nldd-title>
    <nldd-inline-dialog
      v-if="k.grammen.length === 0"
      icon="info"
      text="De kroniek is leeg"
      :supporting-text="k.eigen ? 'Dien als aanvrager een aanvraag in om het eerste chronolexogram te maken.' : k.rechter ? 'Stel als aanvrager beroep in tegen een beslissing op bezwaar.' : undefined"
    ></nldd-inline-dialog>
    <nldd-container v-else gap="24">
      <GramCode
        v-for="(g, i) in k.grammen"
        :id="`gram-${g.id}`"
        :key="g.id"
        tabindex="-1"
        :gram="g"
        :nummer="i + 1"
        :gemarkeerd="g.id === gemarkeerd"
      />
    </nldd-container>
  </nldd-simple-section>
</template>
