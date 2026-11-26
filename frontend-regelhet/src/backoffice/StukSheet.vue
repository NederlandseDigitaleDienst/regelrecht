<script setup lang="ts">
// One record of a chronicle in a sheet, titled in the page with the bar taking
// the title over on scroll: what it holds, with the article it rests on and the
// record it refers to as rows of it, what came out, and for
// a decision of the executor the full chain of why, as the server explains it
// (`GET /api/backoffice/waarom/{id}`). A submission, or a decision on one,
// also leads to its panel in the work queue, unless that panel is the screen
// behind the sheet (`huidig`). Opened from the chronicle and from the timeline
// of an application (KroniekView.vue, BehandelenPaneel.vue).
import { computed, ref, useId, watch } from 'vue';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { useRouter } from 'vue-router';
import { longDate } from '../gedeeld/datum';
import Gegevens from '../gedeeld/Gegevens.vue';
import { foutTekst } from '../gedeeld/http';
import { artikelNaam, isBesluit } from '../gedeeld/keten';
import type { Casus, Gram, Vormen, Waarom } from '../gedeeld/types';
import VolledigeKeten from '../gedeeld/VolledigeKeten.vue';
import { backofficeApi } from './api';
import type { Kroniek } from './api';
import { indieningVan, noemStuk, omschrijfStuk } from './kroniekStuk';

const props = defineProps<{
  stuk: Gram | null;
  kroniek: Kroniek | undefined;
  casus: Casus | null | undefined;
  vormen: Vormen | null | undefined;
  /** The submission whose panel is behind the sheet, which it does not open again. */
  huidig?: string;
}>();
const emit = defineEmits<{ sluit: [] }>();
const router = useRouter();

const noem = (id: string) => noemStuk(id, props.casus, props.kroniek);
const omschrijf = (g: Gram) => omschrijfStuk(g, props.casus, props.kroniek);
const titel = computed(() => (props.stuk && props.casus ? `${omschrijf(props.stuk)} van ${longDate(props.stuk.recorded_on)}` : ''));
const titelId = `${useId()}-titel`;
/** The article the record rests on and the record it refers to, as rows of what it is about. */
const bij = computed(() => {
  const g = props.stuk;
  if (!g) return [];
  return [
    { naam: 'grondslag', label: 'Op grond van', waarde: artikelNaam(g.basis) },
    ...(g.refers_to ? [{ naam: 'verwijzing', label: 'Verwijst naar', waarde: hoofdletter(noem(g.refers_to)) }] : []),
  ];
});
const naarPaneel = (g: Gram) => {
  const op = indieningVan(g, props.kroniek);
  return op && op.id !== props.huidig ? op : undefined;
};

/** Why the decision in the sheet came out so, once loaded; for a decision of the executor only. */
const waarom = ref<Waarom | null>(null);
const waaromFout = ref('');
watch(
  () => props.stuk,
  async (g) => {
    waarom.value = null;
    waaromFout.value = '';
    const k = props.kroniek;
    if (!g || !k || !isBesluit(g, k.stand) || g.recording_actor !== props.casus?.cel) return;
    try {
      const uitleg = await backofficeApi.waarom(g.id);
      if (props.stuk?.id === g.id) waarom.value = uitleg;
    } catch (e) {
      if (props.stuk?.id === g.id) waaromFout.value = foutTekst(e);
    }
  },
  { immediate: true },
);
function naarWerkvoorraad(g: Gram) {
  const op = naarPaneel(g);
  emit('sluit');
  if (op && props.kroniek) void router.push({ name: 'behandelen', params: { cel: props.kroniek.cel, id: op.id } });
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet width="720px" :open="stuk ? true : undefined" @close="emit('sluit')">
      <!-- Per record a page of its own: the bar finds its anchor when it mounts, and the sheet opens at the top. -->
      <nldd-page :key="stuk?.id" sticky-header>
        <nldd-top-title-bar slot="header" heading-level="2" :text="titel" dismiss-text="Sluit" :collapse-anchor="titelId"></nldd-top-title-bar>
        <nldd-simple-section v-if="stuk && casus">
          <nldd-container>
            <nldd-title :id="titelId" size="display-2" heading-level="2" :text="titel"></nldd-title>
            <nldd-spacer size="24"></nldd-spacer>
            <Gegevens
              :waarden="stuk.fields"
              :teksten="vormen?.teksten"
              :volgorde="vormen?.volgorde"
              :noem="noem"
              :bij="bij"
              :kopniveau="3"
              label="Gegevens van het stuk"
            />
            <template v-if="stuk.outputs">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title heading-level="3" size="display-5" text="Uitkomst"></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <Gegevens :waarden="stuk.outputs" :teksten="vormen?.teksten" :volgorde="vormen?.volgorde" label="Uitkomst van het besluit" />
            </template>
            <template v-if="waarom?.keten">
              <nldd-spacer size="16"></nldd-spacer>
              <VolledigeKeten :keten="waarom.keten" :casus="casus" :teksten="vormen?.teksten" />
            </template>
            <template v-if="waaromFout">
              <nldd-spacer size="16"></nldd-spacer>
              <nldd-inline-dialog icon="warning" text="De keten van dit besluit laadt niet" :supporting-text="waaromFout"></nldd-inline-dialog>
            </template>
            <template v-if="naarPaneel(stuk)">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-button-group orientation="vertical">
                <nldd-button appearance="secondary" text="Open in Aanvragen" @click="naarWerkvoorraad(stuk)"></nldd-button>
              </nldd-button-group>
            </template>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
