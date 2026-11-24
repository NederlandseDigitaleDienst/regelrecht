<script setup lang="ts">
// One decision as the back office shows it, before it is taken and after:
// what came out, in the order, groups and labels of the UI spec; what it
// rests on, per open term and per answer of a cell; and folded away, the
// full chain. A preview (`voorlopig`) is titled as such. Whether it grants what was asked
// carries no tag of its own: the output says it in the list, and the button
// that takes it and the line above it name it again. BehandelenPaneel.vue
// shows the preview and the decision taken with this one view, so what the
// decider sees before the button is what the button records.
//
// Above it stands the decision as the message that makes it known
// (gedeeld/Bericht.vue, the employee's view): before the decision as a
// concept, so the decider reads the message the button sends, with every
// part the law requires, what is missing and whether its invariants hold.
import { computed } from 'vue';
import Bericht from '../gedeeld/Bericht.vue';
import BerichtVerzenden from './BerichtVerzenden.vue';
import { verzendingenVan } from '../gedeeld/bericht';
import Gegevens from '../gedeeld/Gegevens.vue';
import type { Casus, Gram, Vormen, Waarom } from '../gedeeld/types';
import VolledigeKeten from '../gedeeld/VolledigeKeten.vue';
import WaaromLijst from '../gedeeld/WaaromLijst.vue';

const props = withDefaults(defineProps<{
  besluit: Pick<Gram, 'outputs' | 'recording_actor'>;
  waarom?: Waarom | null;
  casus: Casus;
  vormen: Vormen | null;
  celNaam: (cel: string) => string;
  /** A preview: what the decision would come to, not recorded. */
  voorlopig?: boolean;
  /** Heading level of the decision's title and of what it rests on. */
  niveau?: number;
  /** The records the pane has, to find the submission the decision answers for the letter. */
  grammen?: Gram[];
}>(), { niveau: 4 });

const emit = defineEmits<{ verzonden: [] }>();
/** The decision as a whole record, when the caller has one: then it shows as a letter. */
const gram = computed(() => ('id' in props.besluit && 'basis' in props.besluit ? (props.besluit as Gram) : null));
/** The last fact of sending of this decision: then the letter is the one that went, frozen. */
const verzonden = computed(() => (gram.value ? (verzendingenVan(gram.value.id, props.grammen ?? [], props.vormen?.berichten).at(-1) ?? null) : null));
</script>

<template>
  <nldd-container>
    <nldd-title :heading-level="niveau" size="display-5" :text="voorlopig ? 'Voorvertoning van het besluit' : 'Het besluit'"></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <template v-if="gram">
      <Bericht
        :gram="gram"
        rol="besluit"
        :vormen="vormen"
        :grammen="grammen"
        :waarom="waarom"
        :afzender="celNaam(gram.recording_actor)"
        :cel-naam="celNaam"
        :concept="voorlopig"
        :verzonden="verzonden"
        weergave="medewerker"
      />
      <template v-if="!voorlopig">
        <nldd-spacer size="16"></nldd-spacer>
        <BerichtVerzenden
          :gram="gram"
          :grammen="grammen ?? []"
          :vormen="vormen"
          :waarom="waarom"
          :cel-naam="celNaam"
          @verzonden="emit('verzonden')"
        />
      </template>
      <nldd-spacer size="16"></nldd-spacer>
    </template>
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
