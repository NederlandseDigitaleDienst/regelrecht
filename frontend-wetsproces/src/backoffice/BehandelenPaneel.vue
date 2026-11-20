<script setup lang="ts">
// One submission, for the decider of the cell that holds it: what was
// submitted, the findings the deciding article asks of the authority
// (`GET /api/formulier`, `besluiten`), a preview of what the decision would
// come to, and the decision itself. The preview may ask more: a finding a
// policy leaves to the authority for this case (`stil`). Once a decision is
// taken it shows with why it came out so. The main pane beside the work queue
// (WerkvoorraadView.vue), which hands it what it loaded.
import { computed, onMounted, ref } from 'vue';
import { zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { schermveld } from '../gedeeld/formulier';
import type { Schermveld } from '../gedeeld/formulier';
import Gegevens from '../gedeeld/Gegevens.vue';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { besluitenOp, celNaam, omschrijving, toegewezen } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Casus, Gram, Ik, Veld, Voorvertoning, Vormen, Waarom } from '../gedeeld/types';
import WaaromLijst from '../gedeeld/WaaromLijst.vue';
import { backofficeApi } from './api';
import type { Kroniek } from './api';

const props = defineProps<{
  cel: string;
  id: string;
  casus: Casus;
  vormen: Vormen | null;
  wie: Ik | null;
  kronieken: Kroniek[];
}>();
const emit = defineEmits<{ besloten: [] }>();

const kroniek = computed(() => props.kronieken.find((k) => k.cel === props.cel));
const indiening = computed(() => kroniek.value?.grammen.find((g) => g.id === props.id) ?? null);
const mag = computed(() => props.wie?.cellen.find((c) => c.cel === props.cel)?.beslissen ?? false);
const naamVanCel = (cel: string) => celNaam(cel, props.casus);
const titel = computed(() => (indiening.value ? omschrijving(indiening.value, props.casus) : 'Indiening'));

/** The decision form of the article the submission asks a decision of. */
const besluitvorm = computed(() => {
  const g = indiening.value;
  const artikel = g && props.vormen?.indieningen[g.basis]?.besluit;
  return artikel ? props.vormen?.besluiten[artikel] : undefined;
});

// The findings the preview found a policy leaves to the authority.
const stil = ref<Veld[]>([]);
const velden = computed<Schermveld[]>(() => {
  const eigen = besluitvorm.value?.bevindingen ?? [];
  const extra = stil.value.filter((s) => !eigen.some((e) => e.naam === s.naam));
  return [...eigen, ...extra].map((v) => schermveld(v, props.vormen?.teksten));
});
const bevindingen = ref<Record<string, unknown>>({});

const voorvertoning = ref<Voorvertoning | null>(null);
const genomen = ref<{ besluit: Gram; waarom: Waarom } | null>(null);
const melding = ref('');
const bezig = ref(false);
let sleutel = nieuweSleutel();

/** A decision already on the submission, taken before this screen opened. */
const eerder = computed(() =>
  indiening.value && kroniek.value ? besluitenOp(indiening.value.id, kroniek.value.grammen, props.casus).at(-1) : undefined,
);
const eerderWaarom = ref<Waarom | null>(null);
const besluit = computed(() => genomen.value?.besluit ?? eerder.value ?? null);
const waarom = computed(() => genomen.value?.waarom ?? eerderWaarom.value);
const stand = (g: Gram) => toegewezen(g, props.casus);

async function laadEerder() {
  if (eerder.value && !genomen.value) eerderWaarom.value = await backofficeApi.waarom(eerder.value.id).catch(() => null);
}
onMounted(laadEerder);

async function voorvertonen() {
  bezig.value = true;
  melding.value = '';
  try {
    const v = await backofficeApi.voorvertoning({ op: props.id, bevindingen: zonderLeeg(bevindingen.value) });
    voorvertoning.value = v;
    for (const s of v.stil) if (!stil.value.some((x) => x.naam === s.naam)) stil.value.push(s);
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}

async function beslissen() {
  bezig.value = true;
  melding.value = '';
  try {
    genomen.value = await backofficeApi.beslissen({ op: props.id, bevindingen: zonderLeeg(bevindingen.value) }, sleutel);
    sleutel = nieuweSleutel();
    emit('besloten');
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <nldd-page landmarks="page" sticky-header :accessible-label="titel">
    <nldd-top-title-bar slot="header" :text="titel" back-text="Werkvoorraad" collapse-anchor="indiening-titel"></nldd-top-title-bar>
    <nldd-simple-section width="full">
      <nldd-inline-dialog v-if="!indiening" icon="info" text="Deze indiening staat niet in een kroniek die u mag lezen"></nldd-inline-dialog>
      <nldd-container v-else gap="24">
        <nldd-title
          id="indiening-titel"
          size="display-3"
          heading-level="3"
          :text="titel"
          :supporting-text="`Ontvangen ${longDate(indiening.recorded_on)} door ${naamVanCel(indiening.recording_actor)} · onder ${indiening.basis}`"
        ></nldd-title>

        <nldd-container gap="8">
          <nldd-title heading-level="4" size="display-5" text="Wat er is ingediend"></nldd-title>
          <Gegevens :waarden="indiening.fields" :teksten="vormen?.teksten" label="Gegevens van de indiening" />
        </nldd-container>

        <nldd-container v-if="besluit" gap="16">
          <nldd-banner
            :variant="genomen ? 'success' : 'neutral'"
            :text="genomen ? 'Het besluit is genomen' : `Op deze indiening is besloten op ${longDate(besluit.recorded_on)}`"
          ></nldd-banner>
          <nldd-title heading-level="4" size="display-5" text="Het besluit">
            <nldd-tag
              v-if="stand(besluit) !== undefined"
              slot="end"
              :text="stand(besluit) ? 'Toegewezen' : 'Afgewezen'"
              :color="stand(besluit) ? 'success' : 'critical'"
            ></nldd-tag>
          </nldd-title>
          <Gegevens :waarden="besluit.outputs" :teksten="vormen?.teksten" label="Uitkomst van het besluit" />
          <nldd-title heading-level="5" size="display-6" text="Waarom"></nldd-title>
          <WaaromLijst v-if="waarom" :waarom="waarom" :teksten="vormen?.teksten" :cel-naam="naamVanCel" />
        </nldd-container>

        <nldd-container v-else-if="mag" gap="16">
          <nldd-title
            heading-level="4"
            size="display-5"
            text="Bevindingen"
            supporting-text="Het artikel laat deze punten aan het bestuursorgaan over. Een oordeel dat u leeg laat, gaat niet mee."
          ></nldd-title>
          <SpecFormulier
            v-if="velden.length"
            :velden="velden"
            :waarden="bevindingen"
            oordeel
            @zet="(n, v) => (bevindingen[n] = v)"
          />
          <nldd-text v-else color="secondary">Dit besluit vraagt het bestuursorgaan geen bevindingen.</nldd-text>

          <nldd-inline-dialog
            v-if="voorvertoning?.geweigerd"
            icon="warning"
            text="Met deze bevindingen is nog geen besluit mogelijk"
            :supporting-text="voorvertoning.geweigerd"
          ></nldd-inline-dialog>
          <nldd-container v-else-if="voorvertoning?.voorlopig" gap="8">
            <nldd-title heading-level="5" size="display-6" text="Voorvertoning">
              <nldd-tag
                v-if="stand(voorvertoning.voorlopig.besluit) !== undefined"
                slot="end"
                :text="stand(voorvertoning.voorlopig.besluit) ? 'Zou toegewezen worden' : 'Zou afgewezen worden'"
                :color="stand(voorvertoning.voorlopig.besluit) ? 'success' : 'critical'"
              ></nldd-tag>
            </nldd-title>
            <Gegevens :waarden="voorvertoning.voorlopig.besluit.outputs" :teksten="vormen?.teksten" label="Uitkomst van de voorvertoning" />
          </nldd-container>

          <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="Dat lukt niet" :supporting-text="melding"></nldd-inline-dialog>
          <nldd-button-group orientation="horizontal">
            <nldd-button appearance="secondary" text="Voorvertoning" :loading="bezig || undefined" @click="voorvertonen"></nldd-button>
            <nldd-button appearance="primary" text="Besluit nemen" :loading="bezig || undefined" @click="beslissen"></nldd-button>
          </nldd-button-group>
        </nldd-container>
        <nldd-inline-dialog v-else icon="info" text="Een beslisser van deze cel neemt het besluit"></nldd-inline-dialog>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
