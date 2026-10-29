<script setup>
// One step of the chain of a decision, as a row of an nldd-list of type
// tree. A step with steps below it is a branch that folds; the depth shows as
// indentation (a spacer cell per level), as the list asks. A tag says what filled the step in (law, policy, a reduction or
// the bestuursorgaan, herkomst.js); a click on the step opens the article or
// the chronolexograms the reduction read. The row is segmented then: the
// chevron folds, the rest opens the sheet.
import { computed, ref } from 'vue';
import { stapTekst } from '../logboek.js';
import { herkomstVan, klikbaar, SOORTEN, toonHerkomst } from '../herkomst.js';

defineOptions({ name: 'KetenStap' });

const props = defineProps({
  // A step from `genummerd()`: { nummer, stap, kinderen }; `nummer` is only
  // its key.
  knoop: { type: Object, required: true },
  diepte: { type: Number, default: 0 },
});

const open = ref(true);
const tak = computed(() => props.knoop.kinderen.length > 0);
const tekst = computed(() => stapTekst(props.knoop.stap));
const herkomst = computed(() => herkomstVan(props.knoop.stap));
const tag = computed(() => herkomst.value && SOORTEN[herkomst.value.soort]);
const knop = computed(() => klikbaar(herkomst.value));
</script>

<template>
  <nldd-list-item
    size="sm"
    :button="(tak && !knop) || undefined"
    :expanded="tak ? open : undefined"
    @click.stop="tak && !knop && (open = !open)"
  >
    <nldd-spacer-cell v-for="n in diepte" :key="n" size="24"></nldd-spacer-cell>
    <nldd-list-item-segment v-if="tak && knop" button disclosure accessible-label="Open of dicht" @click.stop="open = !open">
      <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
    </nldd-list-item-segment>
    <template v-else>
      <nldd-icon-cell v-if="tak" disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
      <nldd-spacer-cell v-else size="16"></nldd-spacer-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
    </template>
    <nldd-list-item-segment v-if="knop" button width="full" @click.stop="toonHerkomst(herkomst)">
      <nldd-text-cell :text="tekst.tekst" :supporting-text="tekst.toelichting"></nldd-text-cell>
      <nldd-cell width="fit-content">
        <nldd-tag size="sm" :color="tag.kleur" :text="tag.tekst"></nldd-tag>
      </nldd-cell>
    </nldd-list-item-segment>
    <template v-else>
      <nldd-text-cell :text="tekst.tekst" :supporting-text="tekst.toelichting"></nldd-text-cell>
      <nldd-cell v-if="tag" width="fit-content">
        <nldd-tag size="sm" :color="tag.kleur" :text="tag.tekst"></nldd-tag>
      </nldd-cell>
    </template>
    <KetenStap
      v-for="k in knoop.kinderen"
      :key="k.nummer"
      slot="children"
      :knoop="k"
      :diepte="diepte + 1"
    />
  </nldd-list-item>
</template>
