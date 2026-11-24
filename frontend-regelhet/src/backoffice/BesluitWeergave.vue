<script setup lang="ts">
// One decision as the back office shows it, before it is taken and after:
// what came out, in the order, groups and labels of the UI spec; why, per
// open term and per answer of a cell; and folded away, the full chain. A
// preview (`voorlopig`) says it is not a decision yet. BehandelenPaneel.vue
// shows the preview and the decision taken with this one view, so what the
// decider sees before the button is what the button records.
import { computed } from 'vue';
import Gegevens from '../gedeeld/Gegevens.vue';
import { toegewezen } from '../gedeeld/keten';
import type { Casus, Gram, Vormen, Waarom } from '../gedeeld/types';
import VolledigeKeten from '../gedeeld/VolledigeKeten.vue';
import WaaromLijst from '../gedeeld/WaaromLijst.vue';

const props = defineProps<{
  besluit: Pick<Gram, 'outputs' | 'recording_actor'>;
  waarom?: Waarom | null;
  casus: Casus;
  vormen: Vormen | null;
  celNaam: (cel: string) => string;
  /** A preview: what the decision would come to, not recorded. */
  voorlopig?: boolean;
}>();

const uitkomst = computed(() => toegewezen(props.besluit, props.casus));
const tag = computed(() => {
  const t = uitkomst.value;
  if (t === undefined) return null;
  if (props.voorlopig) return { tekst: t ? 'Zou toegewezen worden' : 'Zou afgewezen worden', kleur: t ? 'success' : 'critical' };
  return { tekst: t ? 'Toegewezen' : 'Afgewezen', kleur: t ? 'success' : 'critical' };
});
</script>

<template>
  <nldd-container gap="16">
    <nldd-title
      heading-level="4"
      size="display-5"
      :text="voorlopig ? 'Voorvertoning van het besluit' : 'Het besluit'"
      :supporting-text="
        voorlopig
          ? 'Nog geen besluit: er is niets vastgelegd. Zo valt het besluit uit als u nu op ‘Besluit nemen’ klikt, met de bevindingen en de kroniek zoals ze nu zijn.'
          : undefined
      "
    >
      <nldd-tag v-if="tag" slot="end" :text="tag.tekst" :color="tag.kleur"></nldd-tag>
    </nldd-title>
    <nldd-container>
      <Gegevens
        :waarden="besluit.outputs"
        :teksten="vormen?.teksten"
        :volgorde="vormen?.volgorde"
        :label="voorlopig ? 'Uitkomst van de voorvertoning' : 'Uitkomst van het besluit'"
      />
    </nldd-container>
    <template v-if="waarom">
      <nldd-title heading-level="5" size="display-6" text="Waarom"></nldd-title>
      <WaaromLijst :waarom="waarom" :teksten="vormen?.teksten" :cel-naam="celNaam" :beslisser="celNaam(besluit.recording_actor)" />
      <VolledigeKeten v-if="waarom.keten" :keten="waarom.keten" :casus="casus" :teksten="vormen?.teksten" />
    </template>
  </nldd-container>
</template>
