<script setup lang="ts">
// One step of the chain of a decision (chronolex/keten.ts), as a row of an
// nldd-list of type tree. A step with steps below it is a branch that folds;
// the depth shows as indentation (a spacer cell per level), as the list
// asks. A tag says what filled the step in (law, policy, a reduction or the
// bestuursorgaan). When the app can show that origin (`kanOpenen`), a click
// on the step emits it (`kies`); the row is segmented then: the chevron
// folds, the rest opens. Shared by the chronolexografie poc and the
// RegelHet; each says how it names a regelwerk and a field
// (`namen`) and where a regelwerk sits (`laag`).
import { computed, ref } from 'vue';
import { herkomstVan, klikbaar, SOORTEN, stapTekst } from '../../chronolex/keten';
import type { Herkomst, Knoop, Laag, Namen } from '../../chronolex/keten';

defineOptions({ name: 'KetenStap' });

const props = withDefaults(
  defineProps<{
    knoop: Knoop;
    namen: Namen;
    laag: (wet: string) => Laag;
    diepte?: number;
    /** Whether a click on a step with an origin shows that origin. */
    kanOpenen?: boolean;
  }>(),
  { diepte: 0, kanOpenen: false },
);
const emit = defineEmits<{ kies: [herkomst: Herkomst] }>();

const open = ref(true);
const tak = computed(() => props.knoop.kinderen.length > 0);
const tekst = computed(() => stapTekst(props.knoop.stap, props.namen));
const herkomst = computed(() => herkomstVan(props.knoop.stap, props.laag));
const tag = computed(() => (herkomst.value ? SOORTEN[herkomst.value.soort] : undefined));
const knop = computed(() => props.kanOpenen && klikbaar(herkomst.value));
function kies() {
  if (herkomst.value) emit('kies', herkomst.value);
}
</script>

<template>
  <nldd-list-item
    size="sm"
    :button="(tak && !knop) || undefined"
    :expanded="tak ? open : undefined"
    @click.stop="tak && !knop && (open = !open)"
  >
    <nldd-spacer-cell v-for="n in diepte" :key="n" size="24"></nldd-spacer-cell>
    <nldd-list-item-segment v-if="tak && knop" button disclosure accessible-label="Onderliggende stappen" @click.stop="open = !open">
      <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
    </nldd-list-item-segment>
    <template v-else>
      <nldd-icon-cell v-if="tak" disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
      <nldd-spacer-cell v-else size="16"></nldd-spacer-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
    </template>
    <nldd-list-item-segment v-if="knop && tag" button width="full" @click.stop="kies">
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
      :namen="namen"
      :laag="laag"
      :diepte="diepte + 1"
      :kan-openen="kanOpenen"
      @kies="(h: Herkomst) => emit('kies', h)"
    />
  </nldd-list-item>
</template>
