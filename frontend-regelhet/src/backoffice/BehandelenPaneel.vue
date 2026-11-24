<script setup lang="ts">
// One submission, for the decider of the cell that holds it: what was
// submitted, the findings the deciding article asks of the authority
// (`GET /api/formulier`, `besluiten`), the decision as it would be taken, and
// the button that takes it. The preview is the server's (`voorvertoning`): it
// runs when the pane opens and again when a finding changes, the chronicles
// change (a fact recorded, an advice received) or the test clock moves. It
// may ask more: a finding a policy leaves to the authority for this case
// (`stil`). The preview and the decision taken show in one view
// (BesluitWeergave.vue), with why and the full chain. The main pane beside
// the work queue (WerkvoorraadView.vue), which hands it what it loaded.
//
// What answers the submission comes from the server (`stand` of the
// chronicle): its decision stands; another decision took it back and holds a
// new one in its place; or a later record revoked it with nothing in its
// place. Whether it waits for a decision, the server's queue says (`open`:
// the work queue of the executor, or what waits for an external party), and
// nothing else: the form and the button show only for a submission in it.
// When the queue names a record to decide after (`na`), such as the receipt
// of a judgment that annulled a decision on objection, the decider decides
// again after it; after a withdrawal nothing waits.
//
// With `namens` it is the pane beside the admin panel (BeheerView.vue): the
// submission waits for an external party, such as an appeal for the court,
// is one of what the panel shows of that party (`open`), and the admin
// decides on its behalf through the panel's routes.
//
// For the executor it also shows what its policy asked of another party for
// this application, such as the examination by the GGD, and whether the
// answer came back (`GET /api/backoffice/verzoeken`): from the executor's own
// records, never from the party's chronicle. Under it, the timeline of the
// application (gedeeld/TijdlijnLijst.vue, `GET /api/backoffice/tijdlijn/{id}`).
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { weigeringInWoorden } from '@regelrecht/frontend-shared/chronolex/weigering';
import { zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { herkomsten, kaalSchermveld, schermveld } from '../gedeeld/formulier';
import type { Schermveld } from '../gedeeld/formulier';
import Gegevens from '../gedeeld/Gegevens.vue';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { antwoordOp, artikelNaam, celNaam, herroeping as herroepingVan, herroepingTekst, omschrijving, rolVan, toegewezen } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import TijdlijnLijst from '../gedeeld/TijdlijnLijst.vue';
import type { Casus, Gram, Ik, Rol, Tijdlijn, Veld, Voorvertoning, Vormen, Waarom } from '../gedeeld/types';
import { backofficeApi, beheerApi } from './api';
import BesluitWeergave from './BesluitWeergave.vue';
import type { EigenVerzoek, Kroniek, Wachtend } from './api';
import { feitNaam } from './feit';
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
  /**
   * What waits for a decision, as the server's queue says: the executor's
   * work queue, or what waits for the external party. Undefined until it
   * loaded.
   */
  open?: Wachtend[];
  /** Where the back button leads, in words, such as the list the submission was opened from. */
  terug?: string;
}>();
const emit = defineEmits<{ besloten: [] }>();

const kroniek = computed(() => props.kronieken.find((k) => k.cel === props.cel));
/** The submission as the queue holds it, if it waits for a decision. */
const wachtend = computed(() => props.open?.find((w) => w.stuk.id === props.id));
const gevonden = computed(() => (props.namens ? wachtend.value?.stuk : kroniek.value?.grammen.find((g) => g.id === props.id)) ?? null);
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
  const k = props.kronieken.find((x) => x.grammen.some((g) => g.id === id));
  const g = k?.grammen.find((x) => x.id === id);
  return g && k ? noem(g, rolVan(g, k.stand)) : undefined;
};
/** A record in words with its day: `ontvangst van het afschrift van de uitspraak van 20 november 2026`. */
const noem = (g: Gram, rol: Rol | undefined) =>
  `${omschrijving(g, props.casus, rol).toLowerCase()} van ${longDate(g.recorded_on)}`;
const titel = computed(() => (indiening.value ? omschrijving(indiening.value, props.casus, 'indiening') : 'Indiening'));

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

/** What answers the submission now, as the server says; for the executor's own submissions. */
const antwoord = computed(() => (!props.namens && kroniek.value ? antwoordOp(props.id, kroniek.value.stand) : undefined));
/** The last decision on the submission, taken before this screen opened, whether it still stands or not. */
const laatste = computed(() => {
  const a = antwoord.value;
  if (!a || a.soort === 'open') return undefined;
  return kroniek.value?.grammen.find((g) => g.id === a.besluit);
});
/** What revoked the last decision, in a word and in a sentence. */
const herroeping = computed(() => {
  const k = kroniek.value;
  const h = laatste.value && k ? herroepingVan(laatste.value.id, k.stand, k.grammen) : undefined;
  return h && k ? { woord: h.woord, tekst: herroepingTekst(h, props.casus, k.stand), gram: h.gram } : null;
});
/** The decision another record took in the place of the revoked one, which answers the submission now. */
const inDePlaats = computed(() => (antwoord.value?.soort === 'in_de_plaats' ? antwoord.value.nieuw : null));
/** The record to decide after, when the queue says to decide again. */
const na = computed(() => wachtend.value?.na);
/** Whether the submission waits for a decision: when the server's queue holds it, and only then. */
const wacht = computed(() => !!wachtend.value);
/** A decision already on the submission that stands, taken before this screen opened. */
const eerder = computed(() => (antwoord.value?.soort === 'besloten' ? laatste.value : undefined));
const eerderWaarom = ref<Waarom | null>(null);
const besluit = computed(() => genomen.value?.besluit ?? eerder.value ?? null);
const waarom = computed(() => genomen.value?.waarom ?? eerderWaarom.value);
const stand = (g: Pick<Gram, 'outputs'>) => toegewezen(g, props.casus);

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
  return `${gevraagd}. ${omschrijving(antwoord, props.casus, 'vastlegging')} op ${longDate(antwoord.recorded_on)}.`;
}

// The preview follows what it depends on. Only the answer to the last
// request counts: an earlier one that arrives late is dropped.
const voorvertoningFout = ref('');
const rekent = ref(false);
let vraag = 0;
async function voorvertonen() {
  const deze = ++vraag;
  rekent.value = true;
  try {
    const v = await api.voorvertoning({ op: props.id, na: na.value, bevindingen: zonderLeeg(bevindingen.value) });
    if (deze !== vraag) return;
    voorvertoning.value = v;
    voorvertoningFout.value = '';
    for (const s of v.stil) if (!stil.value.some((x) => x.naam === s.naam)) stil.value.push(s);
  } catch (e) {
    if (deze === vraag) voorvertoningFout.value = foutTekst(e);
  } finally {
    if (deze === vraag) rekent.value = false;
  }
}
/**
 * Why no decision can be computed yet, in Dutch: what the decision misses,
 * by the label of the UI spec; for a refusal it has no words for, the cell's
 * own message.
 */
const weigering = computed(() => {
  const fout = voorvertoning.value?.geweigerd;
  if (!fout) return '';
  const woorden = {
    veld: (naam: string) => kaalSchermveld(naam, props.vormen?.teksten).label,
    rol: props.casus.rol ?? 'het bestuursorgaan',
  };
  return weigeringInWoorden(fout, woorden) ?? `Het besluit is nog niet te berekenen. De melding van het systeem, in het Engels: ${fout}`;
});
/** Whether this pane may decide now, and so shows the preview. */
const kanBeslissen = computed(() => !!indiening.value && !genomen.value && !eerder.value && wacht.value && mag.value);
let wachttijd: ReturnType<typeof setTimeout> | undefined;
watch(
  () => [
    kanBeslissen.value,
    props.id,
    na.value,
    JSON.stringify(bevindingen.value),
    props.kronieken.map((k) => k.kop).join(','),
    props.open?.length,
    props.casus.vandaag,
  ],
  () => {
    clearTimeout(wachttijd);
    if (!kanBeslissen.value) return;
    // What shows is stale from now until the new answer is in.
    rekent.value = true;
    // A finding typed in: ask once it settles.
    wachttijd = setTimeout(voorvertonen, voorvertoning.value ? 300 : 0);
  },
  { immediate: true },
);
onUnmounted(() => clearTimeout(wachttijd));

// The timeline of the application, for the executor's own submissions:
// again when a chronicle changes.
const tijdlijn = ref<Tijdlijn | null>(null);
const tijdlijnFout = ref('');
let tijdlijnVraag = 0;
async function laadTijdlijn() {
  if (props.namens) return;
  const deze = ++tijdlijnVraag;
  try {
    const t = await backofficeApi.tijdlijn(props.id);
    if (deze !== tijdlijnVraag) return;
    tijdlijn.value = t;
    tijdlijnFout.value = '';
  } catch (e) {
    if (deze === tijdlijnVraag) tijdlijnFout.value = foutTekst(e);
  }
}
watch(() => props.kronieken.map((k) => k.kop).join(','), laadTijdlijn, { immediate: true });
/**
 * Where a record of the timeline opens: a submission of the executor here,
 * in the work queue; another record of the executor in its chronicle. A
 * record of another cell has no screen on this side.
 */
function naar(g: Gram) {
  if (g.recording_actor !== props.casus.cel) return undefined;
  return tijdlijn.value?.stand.rollen[g.id] === 'indiening'
    ? { name: 'behandelen', params: { cel: g.recording_actor, id: g.id } }
    : { name: 'kroniek', params: { cel: g.recording_actor }, query: { stuk: g.id } };
}

async function beslissen() {
  bezig.value = true;
  melding.value = '';
  try {
    genomen.value = await api.beslissen({ op: props.id, na: na.value, bevindingen: zonderLeeg(bevindingen.value) }, sleutel);
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
    <nldd-top-title-bar slot="header" :text="titel" :back-text="terug ?? (namens ? 'Beheerpaneel' : 'Aanvragen')" collapse-anchor="indiening-titel"></nldd-top-title-bar>
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
          <nldd-container>
            <Gegevens :waarden="indiening.fields" :teksten="vormen?.teksten" :volgorde="vormen?.volgorde" :noem="noemStuk" :herkomst="herkomstVan" label="Gegevens van de indiening" />
          </nldd-container>
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
              <nldd-text-cell :text="omschrijving(v.verzoek, casus, 'vastlegging')" :supporting-text="standVan(v)"></nldd-text-cell>
              <nldd-cell>
                <nldd-tag :text="v.open ? 'Loopt' : 'Antwoord binnen'" :color="v.open ? 'warning' : 'success'" size="sm"></nldd-tag>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-container>

        <nldd-container v-if="laatste && herroeping && !genomen" gap="16">
          <nldd-title heading-level="4" size="display-5" text="Het eerdere besluit">
            <nldd-badge slot="end" :text="herroeping.woord" color="warning" size="sm"></nldd-badge>
          </nldd-title>
          <nldd-banner
            variant="warning"
            :text="`Het besluit van ${longDate(laatste.recorded_on)} geldt niet meer`"
            :supporting-text="`${herroeping.tekst}.`"
          ></nldd-banner>
          <nldd-container>
            <Gegevens :waarden="laatste.outputs" :teksten="vormen?.teksten" label="Uitkomst van het eerdere besluit" />
          </nldd-container>
        </nldd-container>

        <nldd-container v-if="inDePlaats && herroeping && !genomen" gap="16">
          <nldd-title
            heading-level="4"
            size="display-5"
            text="Het geldende besluit"
            :supporting-text="`Genomen in de plaats van het eerdere besluit. ${herroeping.tekst}.`"
          >
            <nldd-tag
              v-if="stand(inDePlaats) !== undefined"
              slot="end"
              :text="stand(inDePlaats) ? 'Toegewezen' : 'Afgewezen'"
              :color="stand(inDePlaats) ? 'success' : 'critical'"
            ></nldd-tag>
          </nldd-title>
          <nldd-container>
            <Gegevens :waarden="inDePlaats.outputs" :teksten="vormen?.teksten" label="Uitkomst van het geldende besluit" />
          </nldd-container>
        </nldd-container>

        <nldd-container v-if="besluit" gap="16">
          <nldd-banner
            :variant="genomen ? 'success' : 'neutral'"
            :text="genomen ? 'Het besluit is genomen' : `Op deze indiening is besloten op ${longDate(besluit.recorded_on)}`"
          ></nldd-banner>
          <BesluitWeergave :besluit="besluit" :waarom="waarom" :casus="casus" :vormen="vormen" :cel-naam="naamVanCel" />
        </nldd-container>

        <nldd-container v-else-if="wacht && mag" gap="16">
          <nldd-banner
            v-if="na"
            variant="neutral"
            text="Opnieuw beslissen"
            :supporting-text="`Het nieuwe besluit volgt op ${noemStuk(na) ?? 'het stuk dat het eerdere besluit vernietigde'}.`"
          ></nldd-banner>
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
            :volgorde="vormen?.volgorde"
            oordeel
            @zet="(n, v) => (bevindingen[n] = v)"
          />
          <nldd-text v-else color="secondary"><p>Dit besluit vraagt {{ naamVanCel(cel) }} geen bevindingen.</p></nldd-text>

          <nldd-inline-dialog
            v-if="voorvertoningFout"
            icon="warning"
            icon-color="critical"
            text="De voorvertoning lukt niet"
            :supporting-text="voorvertoningFout"
          ></nldd-inline-dialog>
          <nldd-inline-dialog v-else-if="!voorvertoning" variant="loading" text="Het besluit wordt berekend"></nldd-inline-dialog>
          <nldd-inline-dialog
            v-else-if="voorvertoning.geweigerd"
            icon="warning"
            text="Er is nog geen besluit te berekenen"
            :supporting-text="weigering"
          ></nldd-inline-dialog>
          <BesluitWeergave
            v-else-if="voorvertoning.voorlopig"
            :besluit="voorvertoning.voorlopig.besluit"
            :waarom="voorvertoning.voorlopig.waarom"
            :casus="casus"
            :vormen="vormen"
            :cel-naam="naamVanCel"
            voorlopig
          />
          <template v-if="voorvertoning?.voorlopig && voorvertoning.automatisch?.length">
            <nldd-title heading-level="5" size="display-6" text="Daarna doet het systeem zelf"></nldd-title>
            <nldd-list appearance="box-base" accessible-label="Wat het systeem daarna zelf vastlegt">
              <nldd-list-item v-for="(a, i) in voorvertoning.automatisch" :key="i" size="md">
                <nldd-text-cell :text="feitNaam(a.feit)" :supporting-text="`Volgens ${artikelNaam(a.artikel)}`"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
          <nldd-text v-if="rekent && voorvertoning" size="sm" color="secondary"><p>De voorvertoning wordt bijgewerkt.</p></nldd-text>

          <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="Dat lukt niet" :supporting-text="melding"></nldd-inline-dialog>
          <nldd-button-group>
            <nldd-button appearance="primary" text="Besluit nemen" :loading="bezig || undefined" @click="beslissen"></nldd-button>
          </nldd-button-group>
        </nldd-container>
        <nldd-inline-dialog v-else-if="wacht" icon="info" text="Een beslisser van deze cel neemt het besluit"></nldd-inline-dialog>

        <TijdlijnLijst
          v-if="!namens"
          :tijdlijn="tijdlijn"
          :fout="tijdlijnFout"
          :casus="casus"
          kant="medewerker"
          :teksten="vormen?.teksten"
          :huidig="id"
          :naar="naar"
        />
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
