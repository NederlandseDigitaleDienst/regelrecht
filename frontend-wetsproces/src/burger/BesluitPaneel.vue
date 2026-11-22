<script setup lang="ts">
// One decision from the citizen's dossier, as the main pane beside it
// (DossierView.vue): what came out, why, and the submissions the citizen can
// make against it: an objection against a decision on an application, an
// appeal to the court against the decision on that objection. Which those
// are follows from the case (keten.ts `indieningenTegen`).
// Its fields come from the law, without what the decision already holds.
import { computed, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter, zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { beginwaarden, tegenBesluitVelden } from '../gedeeld/formulier';
import Gegevens from '../gedeeld/Gegevens.vue';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { artikelNaam, besluitNaam, celNaam, indieningenTegen, isBesluit, termijn, toegewezen } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Casus, CasusIndiening, Gram, Vormen, Waarom } from '../gedeeld/types';
import WaaromLijst from '../gedeeld/WaaromLijst.vue';
import { burgerApi } from './api';

const props = defineProps<{ id: string; casus: Casus; vormen: Vormen | null; dossier: Gram[] }>();
const router = useRouter();
const waarom = ref<Waarom | null>(null);
const waaromFout = ref('');

const besluit = computed(() => {
  const g = props.dossier.find((x) => x.id === props.id);
  return g && isBesluit(g, props.casus) ? g : null;
});
/** The field of a submission against the decision that holds its deadline, as the server derives it from the law. */
const termijnveld = computed(
  () => tegen.value.map((i) => props.vormen?.indieningen[i.artikel]?.termijn).find(Boolean) ?? null,
);
/** The deadline to object or appeal, by the test clock or the real day (`casus.vandaag`). */
const deadline = computed(() => (besluit.value ? termijn(besluit.value, props.casus.vandaag, termijnveld.value) : null));
/** What the UI spec says of a late submission, under the field of the deadline. */
const teLaat = computed(() => (termijnveld.value ? props.vormen?.teksten?.[termijnveld.value]?.help ?? '' : ''));
/** What the citizen can file against it, in words: `bezwaar`, or `bezwaar of beroep`. */
const tegenWoorden = computed(() => tegen.value.map((i) => soortNaam(i).toLowerCase()).join(' of '));
const titel = computed(() =>
  besluit.value ? `${besluitNaam(besluit.value, props.casus)} van ${longDate(besluit.value.recorded_on)}` : 'Besluit',
);
const naamVanCel = (cel: string) => celNaam(cel, props.casus);
const stand = computed(() => (besluit.value ? toegewezen(besluit.value, props.casus) : undefined));
const tegen = computed(() => (besluit.value ? indieningenTegen(besluit.value, props.casus) : []));

watch(
  besluit,
  async (b) => {
    waarom.value = null;
    waaromFout.value = '';
    if (!b) return;
    try {
      waarom.value = await burgerApi.waarom(b.id);
    } catch (e) {
      waaromFout.value = foutTekst(e);
    }
  },
  { immediate: true },
);

// The submission against this decision the sheet holds, with its fields and values.
const open = ref(false);
const keuze = ref<CasusIndiening | null>(null);
const waarden = ref<Record<string, unknown>>({});
const melding = ref('');
const bezig = ref(false);
let sleutel = nieuweSleutel();
const velden = computed(() => {
  const vorm = keuze.value && props.vormen?.indieningen[keuze.value.artikel];
  return vorm && besluit.value ? tegenBesluitVelden(vorm, besluit.value, props.vormen?.teksten) : [];
});
const soortNaam = (i: CasusIndiening) => hoofdletter(i.soort.toLowerCase());

function start(i: CasusIndiening) {
  keuze.value = i;
  melding.value = '';
  sleutel = nieuweSleutel();
  waarden.value = beginwaarden(velden.value);
  open.value = true;
}

async function indienen() {
  if (!keuze.value || !besluit.value) return;
  bezig.value = true;
  melding.value = '';
  try {
    await burgerApi.indienen(
      { artikel: keuze.value.artikel, op: besluit.value.id, velden: zonderLeeg(waarden.value) },
      sleutel,
    );
    open.value = false;
    await router.push({ name: 'dossier', query: { ingediend: keuze.value.soort } });
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <nldd-page landmarks="page" sticky-header :accessible-label="titel">
    <nldd-top-title-bar slot="header" :text="titel" back-text="Mijn dossier" collapse-anchor="besluit-titel"></nldd-top-title-bar>
    <nldd-simple-section width="full">
      <nldd-inline-dialog v-if="!besluit" icon="info" text="Dit besluit staat niet in uw dossier"></nldd-inline-dialog>
      <template v-else>
        <nldd-title id="besluit-titel" size="display-3">
          <h3>{{ titel }}</h3>
          <nldd-tag
            v-if="stand !== undefined"
            slot="end"
            :text="stand ? 'Toegewezen' : 'Afgewezen'"
            :color="stand ? 'success' : 'critical'"
          ></nldd-tag>
        </nldd-title>
        <nldd-text color="secondary">{{ naamVanCel(besluit.recording_actor) }} · onder {{ artikelNaam(besluit.basis) }}</nldd-text>
        <nldd-spacer size="24"></nldd-spacer>

        <nldd-title size="display-5"><h4>Wat eruit kwam</h4></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <Gegevens :waarden="besluit.outputs" :teksten="vormen?.teksten" :volgorde="vormen?.volgorde" label="Uitkomst van het besluit" />
        <nldd-spacer size="24"></nldd-spacer>

        <nldd-title size="display-5"><h4>Waarom</h4></nldd-title>
        <nldd-text color="secondary">Wie de wet op elk punt invulde, en welke antwoorden het besluit gebruikte.</nldd-text>
        <nldd-spacer size="8"></nldd-spacer>
        <WaaromLijst
          v-if="waarom && besluit"
          :waarom="waarom"
          :teksten="vormen?.teksten"
          :cel-naam="naamVanCel"
          :beslisser="naamVanCel(besluit.recording_actor)"
        />
        <nldd-inline-dialog v-else-if="waaromFout" icon="warning" text="De uitleg laadt niet" :supporting-text="waaromFout"></nldd-inline-dialog>
        <nldd-activity-indicator v-else></nldd-activity-indicator>

        <template v-if="tegen.length">
          <nldd-spacer size="24"></nldd-spacer>
          <nldd-title size="display-5"><h4>Bent u het niet eens met dit besluit?</h4></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <template v-if="deadline">
            <nldd-inline-dialog
              v-if="deadline.voorbij"
              icon="warning"
              :text="`De termijn liep tot en met ${longDate(deadline.einde)}`"
              :supporting-text="[`U kunt nog ${tegenWoorden} indienen, maar het is dan te laat.`, teLaat].filter(Boolean).join(' ')"
            ></nldd-inline-dialog>
            <nldd-text v-else>U kunt {{ tegenWoorden }} indienen tot en met {{ longDate(deadline.einde) }}.</nldd-text>
            <nldd-spacer size="8"></nldd-spacer>
          </template>
          <nldd-button-group orientation="horizontal">
            <nldd-button
              v-for="i in tegen"
              :key="i.artikel"
              appearance="secondary"
              :text="`${soortNaam(i)} indienen`"
              @click="start(i)"
            ></nldd-button>
          </nldd-button-group>
        </template>
      </template>
    </nldd-simple-section>
  </nldd-page>

  <Teleport to="body">
    <nldd-sheet width="720px" :open="open || undefined" @close="open = false">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar
            heading-level="2"
            :text="keuze ? `${soortNaam(keuze)} tegen het besluit` : ''"
            dismiss-text="Sluiten"
          ></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section v-if="keuze">
          <nldd-container gap="16">
            <nldd-text color="secondary">
              De vragen komen uit de wet. Wat het besluit al vastlegt, zoals de termijn, neemt
              {{ naamVanCel(keuze.cel) }} uit het besluit zelf.
            </nldd-text>
            <SpecFormulier :velden="velden" :waarden="waarden" :volgorde="vormen?.volgorde" @zet="(n, v) => (waarden[n] = v)" />
            <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="Niet ingediend" :supporting-text="melding"></nldd-inline-dialog>
            <nldd-button-group orientation="horizontal">
              <nldd-button appearance="primary" :text="`${soortNaam(keuze)} indienen`" :loading="bezig || undefined" @click="indienen"></nldd-button>
              <nldd-button appearance="neutral-transparent" text="Annuleren" @click="open = false"></nldd-button>
            </nldd-button-group>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
