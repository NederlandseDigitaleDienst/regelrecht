<script setup lang="ts">
// One decision as the back office shows it, before it is taken and after:
// what came out, in the order, groups and labels of the UI spec; what it
// rests on, per open term and per answer of a cell; and folded away, the
// full chain. A preview (`voorlopig`) is titled as such. Whether it grants what was asked
// carries no tag of its own: the output says it in the list, and the button
// that takes it and the line above it name it again. BehandelenPaneel.vue
// shows the preview and the decision taken with this one view, so what the
// decider sees before the button is what the button records.
import Gegevens from '../gedeeld/Gegevens.vue';
import type { Casus, Gram, Vormen, Waarom } from '../gedeeld/types';
import VolledigeKeten from '../gedeeld/VolledigeKeten.vue';
import WaaromLijst from '../gedeeld/WaaromLijst.vue';

withDefaults(defineProps<{
  besluit: Pick<Gram, 'outputs' | 'recording_actor'>;
  waarom?: Waarom | null;
  casus: Casus;
  vormen: Vormen | null;
  celNaam: (cel: string) => string;
  /** A preview: what the decision would come to, not recorded. */
  voorlopig?: boolean;
  /** Heading level of the decision's title and of what it rests on. */
  niveau?: number;
}>(), { niveau: 4 });
</script>

<template>
  <nldd-container>
    <nldd-title :heading-level="niveau" size="display-5" :text="voorlopig ? 'Voorvertoning van het besluit' : 'Het besluit'"></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <Gegevens
      :waarden="besluit.outputs"
      :teksten="vormen?.teksten"
      :volgorde="vormen?.volgorde"
      :label="voorlopig ? 'Uitkomst van de voorvertoning' : 'Uitkomst van het besluit'"
    />
    <template v-if="waarom">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title :heading-level="niveau" size="display-5" text="Waarop het besluit rust"></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <WaaromLijst :waarom="waarom" :teksten="vormen?.teksten" :cel-naam="celNaam" :beslisser="celNaam(besluit.recording_actor)" />
      <template v-if="waarom.keten">
        <nldd-spacer size="16"></nldd-spacer>
        <VolledigeKeten :keten="waarom.keten" :casus="casus" :teksten="vormen?.teksten" />
      </template>
    </template>
  </nldd-container>
</template>
