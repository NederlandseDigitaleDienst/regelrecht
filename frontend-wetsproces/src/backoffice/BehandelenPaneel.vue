<script setup lang="ts">
// One submission, for the decider of the cell that holds it: what was
// submitted, the findings the deciding article asks of the authority
// (`GET /api/formulier`, `besluiten`), a preview of what the decision would
// come to, and the decision itself. The preview may ask more: a finding a
// policy leaves to the authority for this case (`stil`). Once a decision is
// taken it shows with why it came out so. The main pane beside the work queue
// (WerkvoorraadView.vue), which hands it what it loaded.
//
// With `namens` it is the pane beside the admin panel (BeheerView.vue): the
// submission waits for an external party, such as an appeal for the court,
// is one of what the panel shows of that party (`open`), and the admin
// decides on its behalf through the panel's routes.
//
// For the executor it also shows what its policy asked of another party for
// this application, such as the examination by the GGD, and whether the
// answer came back (`GET /api/backoffice/verzoeken`): from the executor's own
// records, never from the party's chronicle.
import { computed, onMounted, ref, watch } from 'vue';
import { zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { herkomsten, schermveld } from '../gedeeld/formulier';
import type { Schermveld } from '../gedeeld/formulier';
import Gegevens from '../gedeeld/Gegevens.vue';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { artikelNaam, besluitenOp, celNaam, omschrijving, toegewezen } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Casus, Gram, Ik, Veld, Voorvertoning, Vormen, Waarom } from '../gedeeld/types';
import WaaromLijst from '../gedeeld/WaaromLijst.vue';
import { backofficeApi, beheerApi } from './api';
import type { EigenVerzoek, Kroniek } from './api';
import { verzoekenBij } from './verzoek';
import type { VerzoekBij } from './verzoek';

const props = defineProps<{
  cel: string;
  id: string;
  casus: Casus;
  vormen: Vormen | null;
  wie: Ik | null;
  kronieken: Kroniek[];
  /** Decide on behalf of an external party, on one of its waiting submissions. */
  namens?: boolean;
  open?: Gram[];
}>();
const emit = defineEmits<{ besloten: [] }>();

const kroniek = computed(() => props.kronieken.find((k) => k.cel === props.cel));
const gevonden = computed(() => (props.namens ? props.open : kroniek.value?.grammen)?.find((g) => g.id === props.id) ?? null);
// A submission the panel decided on leaves what waits for the party; the pane
// keeps showing it, with the decision.
const onthouden = ref<Gram | null>(null);
watch(gevonden, (g) => (onthouden.value = g ?? onthouden.value), { immediate: true });
const indiening = computed(() => gevonden.value ?? onthouden.value);
/** Per field the system filled in the submission, where its value came from: for the case's application only. */
const herkomstVan = computed(() =>
  indiening.value?.basis === props.casus.artikel ? herkomsten(props.vormen?.indieningen[props.casus.artikel]) : {},
);
const mag = computed(() =>
  props.namens ? (props.wie?.partijen_invoeren ?? false) : (props.wie?.cellen.find((c) => c.cel === props.cel)?.beslissen ?? false),
);
const api = props.namens ? beheerApi : backofficeApi;
const naamVanCel = (cel: string) => celNaam(cel, props.casus);
/**
 * A record a field refers to, in words: `besluit (beschikking) van 13 oktober
 * 2026`. Searched in every chronicle the panel has: an appeal sits with the
 * court, the decision it contests with the municipality.
 */
const noemStuk = (id: string) => {
  const g = props.kronieken.flatMap((k) => k.grammen).find((x) => x.id === id);
  return g ? `${omschrijving(g, props.casus).toLowerCase()} van ${longDate(g.recorded_on)}` : undefined;
};
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
  !props.namens && indiening.value && kroniek.value ? besluitenOp(indiening.value.id, kroniek.value.grammen, props.casus).at(-1) : undefined,
);
const eerderWaarom = ref<Waarom | null>(null);
const besluit = computed(() => genomen.value?.besluit ?? eerder.value ?? null);
const waarom = computed(() => genomen.value?.waarom ?? eerderWaarom.value);
const stand = (g: Gram) => toegewezen(g, props.casus);

async function laadEerder() {
  if (eerder.value && !genomen.value) eerderWaarom.value = await backofficeApi.waarom(eerder.value.id).catch(() => null);
}
onMounted(laadEerder);

// The requests of the executor's policy, for the executor's own submissions.
const alleVerzoeken = ref<EigenVerzoek[]>([]);
const verzoekenFout = ref('');
async function laadVerzoeken() {
  if (props.namens) return;
  try {
    alleVerzoeken.value = (await backofficeApi.verzoeken()).verzoeken;
    verzoekenFout.value = '';
  } catch (e) {
    verzoekenFout.value = foutTekst(e);
  }
}
onMounted(laadVerzoeken);

/** The requests this submission waits on ([`verzoekenBij`]). */
const verzoeken = computed(() => (indiening.value ? verzoekenBij(indiening.value, alleVerzoeken.value) : []));

/** What became of a request, in words. */
function standVan(v: VerzoekBij): string {
  const gevraagd = `Aan ${v.aan_naam}, op ${longDate(v.verzoek.recorded_on)}${v.eerder ? ', bij een eerdere aanvraag' : ''}`;
  const antwoord = v.antwoord_ontvangen;
  if (!antwoord) return `${gevraagd}. Het antwoord is nog niet binnen.`;
  return `${gevraagd}. ${omschrijving(antwoord, props.casus)} op ${longDate(antwoord.recorded_on)}.`;
}

async function voorvertonen() {
  bezig.value = true;
  melding.value = '';
  try {
    const v = await api.voorvertoning({ op: props.id, bevindingen: zonderLeeg(bevindingen.value) });
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
    genomen.value = await api.beslissen({ op: props.id, bevindingen: zonderLeeg(bevindingen.value) }, sleutel);
    sleutel = nieuweSleutel();
    emit('besloten');
    await laadVerzoeken();
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <nldd-page landmarks="page" sticky-header :accessible-label="titel">
    <nldd-top-title-bar slot="header" :text="titel" :back-text="namens ? 'Beheerpaneel' : 'Werkvoorraad'" collapse-anchor="indiening-titel"></nldd-top-title-bar>
    <nldd-simple-section width="full">
      <nldd-inline-dialog
        v-if="!indiening"
        icon="info"
        :text="namens ? 'Deze indiening wacht niet op een externe partij' : 'Deze indiening staat niet in een kroniek die u mag lezen'"
      ></nldd-inline-dialog>
      <nldd-container v-else gap="24">
        <nldd-banner
          v-if="namens"
          variant="neutral"
          :text="`U voert dit besluit in namens ${naamVanCel(cel)}`"
          supporting-text="De kroniek legt vast dat het via het beheerpaneel is ingevoerd, door uw account."
        ></nldd-banner>
        <nldd-title
          id="indiening-titel"
          size="display-3"
          heading-level="3"
          :text="titel"
          :supporting-text="`Ontvangen ${longDate(indiening.recorded_on)} door ${naamVanCel(indiening.recording_actor)} · onder ${artikelNaam(indiening.basis)}`"
        ></nldd-title>

        <nldd-container gap="8">
          <nldd-title heading-level="4" size="display-5" text="Wat er is ingediend"></nldd-title>
          <Gegevens :waarden="indiening.fields" :teksten="vormen?.teksten" :noem="noemStuk" :herkomst="herkomstVan" label="Gegevens van de indiening" />
        </nldd-container>

        <nldd-inline-dialog
          v-if="verzoekenFout"
          icon="warning"
          icon-color="critical"
          text="Wat aan anderen is gevraagd, laadt niet"
          :supporting-text="verzoekenFout"
        ></nldd-inline-dialog>
        <nldd-container v-if="verzoeken.length" gap="8">
          <nldd-title heading-level="4" size="display-5" text="Gevraagd aan anderen"></nldd-title>
          <nldd-list appearance="box-base" accessible-label="Gevraagd aan anderen">
            <nldd-list-item v-for="v in verzoeken" :key="v.verzoek.id" size="md">
              <nldd-text-cell :text="omschrijving(v.verzoek, casus)" :supporting-text="standVan(v)"></nldd-text-cell>
              <nldd-cell>
                <nldd-tag :text="v.open ? 'Loopt' : 'Antwoord binnen'" :color="v.open ? 'warning' : 'success'" size="sm"></nldd-tag>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>
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
          <WaaromLijst
            v-if="waarom && besluit"
            :waarom="waarom"
            :teksten="vormen?.teksten"
            :cel-naam="naamVanCel"
            :beslisser="naamVanCel(besluit.recording_actor)"
          />
        </nldd-container>

        <nldd-container v-else-if="mag" gap="16">
          <nldd-title
            heading-level="4"
            size="display-5"
            text="Bevindingen"
            :supporting-text="`Het artikel laat deze punten aan ${naamVanCel(cel)} over. Een oordeel dat u leeg laat, gaat niet mee.`"
          ></nldd-title>
          <SpecFormulier
            v-if="velden.length"
            :velden="velden"
            :waarden="bevindingen"
            oordeel
            @zet="(n, v) => (bevindingen[n] = v)"
          />
          <nldd-text v-else color="secondary">Dit besluit vraagt {{ naamVanCel(cel) }} geen bevindingen.</nldd-text>

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
