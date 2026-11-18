<script setup lang="ts">
/**
 * UploadConfirmDialog - de bevestigingsstap tussen "bestand gekozen" en
 * "bestand verstuurd" bij een werkdocument-upload.
 *
 * Waarom deze stap bestaat: bij sommige formaten leest een taalmodel het
 * document. Dat mag de gebruiker weten vóórdat het gebeurt, en waar er een
 * echte keuze is (een formaat dat ook zonder AI om te zetten is) hoort hij die
 * keuze te krijgen — standaard uit. De dialoog komt ná de bestandskeuze omdat
 * de tekst dan concreet kan zijn over dít bestand in plaats van over uploads in
 * het algemeen.
 *
 * De indeling waar de tekst op leunt komt van de server (zie
 * `lib/uploadFormats.js`), niet uit een lijst hier.
 */
import { ref, computed, watch, nextTick } from 'vue';
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import {
  loadUploadFormats,
  classifyUpload,
  PASSTHROUGH,
  DETERMINISTIC,
  LLM_ONLY,
  UNKNOWN,
} from '../lib/uploadFormats';
import type { UploadFormats } from '../lib/uploadFormats';

const props = withDefaults(
  defineProps<{
    /** Het gekozen bestand dat op bevestiging wacht; `null` = dialoog dicht. */
    // Only `name` is read; callers pass a File.
    file?: { name: string } | null;
  }>(),
  { file: null },
);
const emit = defineEmits<{
  confirm: [choice: { allowLlm: boolean }];
  cancel: [];
}>();

const dialogEl = ref<NLDDModalDialog | null>(null);
const formats = ref<UploadFormats | null>(null);
const allowLlm = ref(false);

const filename = computed(() => props.file?.name ?? '');
const kind = computed(() => (props.file ? classifyUpload(filename.value, formats.value) : UNKNOWN));

// Alleen waar er iets te kiezen valt. Bij markdown gebeurt er niets met de
// inhoud, dus is er ook niets toe te staan.
const showCheckbox = computed(() => kind.value !== PASSTHROUGH);

// Een formaat dat alleen met AI kan, kán niet zonder toestemming: dan is
// "Uploaden" pas zinnig als het vinkje aan staat. De backend weigert zo'n
// upload ook (400), maar de knop uit laten staan is een eerlijker antwoord dan
// een foutmelding achteraf.
const requiresLlm = computed(() => kind.value === LLM_ONLY);
const canSubmit = computed(() => !requiresLlm.value || allowLlm.value);

const supportingText = computed(() => {
  switch (kind.value) {
    case PASSTHROUGH:
      return 'Dit bestand wordt direct opgeslagen als werkdocument. Er is geen conversie nodig, dus er komt geen AI aan te pas.';
    case DETERMINISTIC:
      return 'Dit bestand wordt normaal zonder AI omgezet naar markdown. Lukt dat niet — bijvoorbeeld bij een gescande pdf zonder tekstlaag — dan stopt de conversie met een melding, in plaats van het alsnog aan een taalmodel te geven.';
    case LLM_ONLY:
      return 'Dit formaat kan alleen met AI worden omgezet: een taalmodel leest het document. Zet je het vinkje niet aan, upload het bestand dan als PDF, Word (.docx) of markdown.';
    default:
      return 'We konden niet vaststellen of dit bestand zonder AI om te zetten is. Zonder vinkje wordt er geen taalmodel gebruikt; lukt de omzetting dan niet, dan stopt de conversie met een melding.';
  }
});

// De keuze reset per bestand: een eerder gegeven toestemming mag niet
// ongemerkt blijven staan voor de volgende upload.
watch(
  () => props.file,
  async (file) => {
    allowLlm.value = false;
    if (!file) {
      dialogEl.value?.hide?.();
      return;
    }
    // Ophalen mag pas nu: zonder upload heeft de editor deze lijst niet nodig.
    // De dialoog opent alvast — de tekst volgt zodra de indeling binnen is, en
    // valt terug op de behoedzame "onbekend"-tekst als dat niet lukt.
    loadUploadFormats().then((f) => { formats.value = f; });
    await nextTick();
    dialogEl.value?.show?.();
  },
  // Ook meteen bij mounten: een aanroeper die de dialoog met een bestand
  // erin monteert (of hem hermonteert terwijl er een wacht) moet 'm zien.
  { immediate: true },
);

function onConfirm() {
  if (!canSubmit.value) return;
  emit('confirm', { allowLlm: allowLlm.value });
}

// Escape / klik naast de dialoog telt als afzien van de upload. Alleen melden
// zolang er nog een bestand wacht: het sluiten ná een bevestiging is ons eigen
// `hide()` en zou anders een tweede, verwarrend annuleersignaal geven.
function onClose() {
  if (props.file) emit('cancel');
}

function onAllowLlmChange(event: Event) {
  allowLlm.value = (event as CustomEvent<{ checked: boolean }>).detail.checked;
}
</script>

<template>
  <Teleport to="body">
    <nldd-modal-dialog
      ref="dialogEl"
      icon="upload-to-cloud"
      text="Document uploaden"
      :supporting-text="supportingText"
      @close="onClose"
    >
      <nldd-container padding="8">
        <nldd-text-cell text="Bestand" :supporting-text="filename"></nldd-text-cell>
        <template v-if="showCheckbox">
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-checkbox-field
            data-testid="upload-confirm-llm"
            label="Omzetten met AI toestaan"
            :checked="allowLlm || undefined"
            @change="onAllowLlmChange"
          ></nldd-checkbox-field>
        </template>
      </nldd-container>
      <nldd-button
        slot="actions"
        appearance="primary"
        text="Uploaden"
        data-testid="upload-confirm-submit"
        :disabled="!canSubmit || undefined"
        @click="onConfirm"
      ></nldd-button>
      <nldd-button
        slot="actions"
        appearance="secondary"
        text="Annuleren"
        data-testid="upload-confirm-cancel"
        @click="emit('cancel')"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>
