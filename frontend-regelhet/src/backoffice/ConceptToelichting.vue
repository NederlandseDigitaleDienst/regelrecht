<script setup lang="ts">
// A draft explanation of a decision taken, in plain language, by Vlam
// (`POST /api/backoffice/concepttoelichting`). Vlam only proposes text: the
// decider edits it here, copies it or discards it. Nothing is recorded and
// the decision does not change. BehandelenPaneel.vue shows this under a
// decision when `/api/ik` says the account may ask for it.
import { ref } from 'vue';
import { foutTekst } from '../gedeeld/http';
import { backofficeApi } from './api';

const props = defineProps<{ besluit: string }>();

const concept = ref<string | null>(null);
const model = ref('');
const bezig = ref(false);
const fout = ref<string | null>(null);
const gekopieerd = ref(false);

async function opstellen() {
  bezig.value = true;
  fout.value = null;
  gekopieerd.value = false;
  try {
    const a = await backofficeApi.concepttoelichting(props.besluit);
    concept.value = a.concept;
    model.value = a.model;
  } catch (e) {
    fout.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}

async function kopieren() {
  if (concept.value === null) return;
  try {
    await navigator.clipboard.writeText(concept.value);
    gekopieerd.value = true;
  } catch (e) {
    fout.value = foutTekst(e);
  }
}

function verwerpen() {
  concept.value = null;
  gekopieerd.value = false;
}

/** The value of an nldd text field from its input event. */
const waarde = (e: Event): string => {
  const detail = (e as CustomEvent<{ value?: unknown }>).detail;
  if (detail && typeof detail === 'object' && 'value' in detail) return String(detail.value ?? '');
  return (e.target as HTMLTextAreaElement | null)?.value ?? '';
};
</script>

<template>
  <nldd-container gap="16">
    <nldd-title
      heading-level="5"
      size="display-6"
      text="Concepttoelichting"
      supporting-text="Een taalmodel (Vlam) stelt een toelichting in gewone taal voor, uit de uitkomst en het waarom van dit besluit. Velden die een BSN, naam of adres heten gaan niet mee. De overige feiten van het besluit gaan wel mee, ook gezondheidsgegevens en vrije tekst die daarin staan."
    ></nldd-title>

    <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" text="Er is geen concept" :supporting-text="fout"></nldd-inline-dialog>

    <nldd-button-group v-if="concept === null">
      <nldd-button appearance="secondary" text="Concepttoelichting laten opstellen" :loading="bezig || undefined" @click="opstellen"></nldd-button>
    </nldd-button-group>

    <template v-else>
      <nldd-banner
        variant="warning"
        text="Concept van een taalmodel"
        :supporting-text="`Deze tekst is geschreven door ${model || 'Vlam'} en niet gecontroleerd. Lees en pas hem aan voordat u hem gebruikt. Het besluit verandert er niet door en er wordt niets vastgelegd.`"
      ></nldd-banner>
      <nldd-form-field label="Toelichting (concept)">
        <nldd-multi-line-text-field :value="concept" rows="10" resize="auto" @input="concept = waarde($event)"></nldd-multi-line-text-field>
      </nldd-form-field>
      <nldd-button-group>
        <nldd-button appearance="primary" :text="gekopieerd ? 'Gekopieerd' : 'Kopiëren'" @click="kopieren"></nldd-button>
        <nldd-button appearance="secondary" text="Opnieuw opstellen" :loading="bezig || undefined" @click="opstellen"></nldd-button>
        <nldd-button appearance="neutral-transparent" text="Verwerpen" @click="verwerpen"></nldd-button>
      </nldd-button-group>
    </template>
  </nldd-container>
</template>
