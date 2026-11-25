<script setup lang="ts">
// One decision as the back office shows it, before it is taken and after,
// in short: whether it grants what was asked (`toegewezen`, from the
// caller), what came out in the order, groups and labels of the UI spec,
// the article it is taken under, and what it rests on, per open term with
// its article and per answer of a cell; folded away, the full chain. A
// preview (`voorlopig`) is titled as such. BehandelenPaneel.vue shows the
// preview and the decision taken with this one view, so what the decider
// sees before the button is what the button records.
//
// The message that makes a decision known (gedeeld/Bericht.vue) is no part
// of it. Once taken, the decision has the button that sends that message
// (BerichtVerzenden.vue), and a link that opens it in a sheet for whoever
// wants to read it first.
import { computed, ref } from 'vue';
import Bericht from '../gedeeld/Bericht.vue';
import BerichtVerzenden from './BerichtVerzenden.vue';
import { berichtsoortVoor, verzendingenVan } from '../gedeeld/bericht';
import Gegevens from '../gedeeld/Gegevens.vue';
import { artikelNaam } from '../gedeeld/keten';
import type { Casus, Gram, Vormen, Waarom } from '../gedeeld/types';
import VolledigeKeten from '../gedeeld/VolledigeKeten.vue';
import WaaromLijst from '../gedeeld/WaaromLijst.vue';

const props = withDefaults(defineProps<{
  besluit: Pick<Gram, 'outputs' | 'recording_actor' | 'basis'>;
  waarom?: Waarom | null;
  casus: Casus;
  vormen: Vormen | null;
  celNaam: (cel: string) => string;
  /** A preview: what the decision would come to, not recorded. */
  voorlopig?: boolean;
  /** Whether it grants the application; undefined when it decides no application, such as an objection. */
  toegewezen?: boolean;
  /** Heading level of the decision's title and of what it rests on. */
  niveau?: number;
  /** The records the pane has, to find the submission the decision answers for the message. */
  grammen?: Gram[];
}>(), { niveau: 4, toegewezen: undefined });

const emit = defineEmits<{ verzonden: [] }>();
/** The decision as a whole record, once taken: then its message can be sent. */
const gram = computed(() => (!props.voorlopig && 'id' in props.besluit ? (props.besluit as Gram) : null));
/** The last fact of sending of this decision: then the message is the one that went, frozen. */
const verzonden = computed(() => (gram.value ? (verzendingenVan(gram.value.id, props.grammen ?? [], props.vormen?.berichten).at(-1) ?? null) : null));
/** Whether the decision has a message to show: one sent, or a kind of message on the template for it. */
const heeftBrief = computed(() => !!gram.value && (!!verzonden.value || !!berichtsoortVoor(gram.value, 'besluit', props.vormen?.berichten)));
/** Whether the sheet with the message is open. */
const brief = ref(false);
</script>

<template>
  <nldd-container>
    <nldd-title :heading-level="niveau" size="display-5" :text="voorlopig ? 'Voorvertoning van het besluit' : 'Het besluit'">
      <nldd-tag
        v-if="toegewezen !== undefined"
        slot="end"
        :text="toegewezen ? 'Toegewezen' : 'Afgewezen'"
        :color="toegewezen ? 'success' : 'critical'"
      ></nldd-tag>
    </nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <Gegevens
      :waarden="besluit.outputs"
      :teksten="vormen?.teksten"
      :volgorde="vormen?.volgorde"
      :bij="[{ naam: 'grondslag', label: 'Op grond van', waarde: artikelNaam(besluit.basis) }]"
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
    <template v-if="gram && vormen?.berichten">
      <nldd-spacer size="24"></nldd-spacer>
      <BerichtVerzenden
        :gram="gram"
        :grammen="grammen ?? []"
        :vormen="vormen"
        :waarom="waarom"
        :cel-naam="celNaam"
        @verzonden="emit('verzonden')"
      />
      <template v-if="heeftBrief">
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-text>
          <p><nldd-link><a href="#brief" @click.prevent="brief = true">Toon brief</a></nldd-link></p>
        </nldd-text>
        <Teleport to="body">
          <nldd-sheet width="720px" accessible-label="Brief" :open="brief || undefined" @close="brief = false">
            <nldd-page>
              <nldd-top-title-bar slot="header" heading-level="2" text="Brief" dismiss-text="Sluit"></nldd-top-title-bar>
              <nldd-simple-section>
                <Bericht
                  v-if="brief"
                  :gram="gram"
                  rol="besluit"
                  :vormen="vormen"
                  :grammen="grammen"
                  :waarom="waarom"
                  :afzender="celNaam(gram.recording_actor)"
                  :cel-naam="celNaam"
                  :verzonden="verzonden"
                  weergave="medewerker"
                />
              </nldd-simple-section>
            </nldd-page>
          </nldd-sheet>
        </Teleport>
      </template>
    </template>
  </nldd-container>
</template>
