<script setup lang="ts">
// One decision from the citizen's dossier: what came out, why, and the
// submissions the citizen can make against it, such as an objection. Which
// those are follows from the case: a submission that refers to a decision
// and that the cell that took it decides on (keten.ts `indieningenTegen`).
// Its fields come from the law, without what the decision already holds.
import { computed, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { beginwaarden, tegenBesluitVelden } from '../gedeeld/formulier';
import Gegevens from '../gedeeld/Gegevens.vue';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { celNaam, indieningenTegen, isBesluit, toegewezen } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { CasusIndiening, Waarom } from '../gedeeld/types';
import WaaromLijst from '../gedeeld/WaaromLijst.vue';
import { burgerApi } from './api';
import { useBurger } from './useBurger';

const props = defineProps<{ id: string }>();
const router = useRouter();
const { casus, vormen, dossier, fout, laden, laad } = useBurger();
const waarom = ref<Waarom | null>(null);
const waaromFout = ref('');

const besluit = computed(() => {
  const c = casus.value;
  const g = dossier.value.find((x) => x.id === props.id);
  return c && g && isBesluit(g, c) ? g : null;
});
const naamVanCel = (cel: string) => (casus.value ? celNaam(cel, casus.value) : cel);
const stand = computed(() => (besluit.value && casus.value ? toegewezen(besluit.value, casus.value) : undefined));
const tegen = computed(() => (besluit.value && casus.value ? indieningenTegen(besluit.value, casus.value) : []));

onMounted(laad);
watch(besluit, async (b) => {
  if (!b) return;
  try {
    waarom.value = await burgerApi.waarom(b.id);
  } catch (e) {
    waaromFout.value = foutTekst(e);
  }
});

// The submission against this decision the sheet holds, with its fields and values.
const open = ref(false);
const keuze = ref<CasusIndiening | null>(null);
const waarden = ref<Record<string, unknown>>({});
const melding = ref('');
const bezig = ref(false);
let sleutel = nieuweSleutel();
const velden = computed(() => {
  const vorm = keuze.value && vormen.value?.indieningen[keuze.value.artikel];
  return vorm && besluit.value ? tegenBesluitVelden(vorm, besluit.value, vormen.value?.teksten) : [];
});
const soortNaam = (i: CasusIndiening) => i.soort.charAt(0) + i.soort.slice(1).toLowerCase();

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
    await router.push({ name: 'dossier', query: { ingediend: '1' } });
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <nldd-simple-section>
    <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Het besluit laadt niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-inline-dialog v-else-if="!besluit" icon="info" text="Dit besluit staat niet in uw dossier">
      <nldd-button slot="actions" appearance="secondary" text="Naar mijn dossier" @click="router.push('/burger')"></nldd-button>
    </nldd-inline-dialog>
    <nldd-container v-else gap="24">
      <nldd-title
        heading-level="1"
        :text="`Besluit van ${longDate(besluit.recorded_on)}`"
        :supporting-text="`${naamVanCel(besluit.recording_actor)} · onder ${besluit.basis}`"
      >
        <nldd-tag
          v-if="stand !== undefined"
          slot="end"
          :text="stand ? 'Toegewezen' : 'Afgewezen'"
          :color="stand ? 'success' : 'critical'"
        ></nldd-tag>
      </nldd-title>

      <nldd-container gap="8">
        <nldd-title heading-level="2" size="display-6" text="Wat eruit kwam"></nldd-title>
        <Gegevens :waarden="besluit.outputs" :teksten="vormen?.teksten" label="Uitkomst van het besluit" />
      </nldd-container>

      <nldd-container gap="8">
        <nldd-title heading-level="2" size="display-6" text="Waarom" supporting-text="Wie elk open begrip in de wet invulde, en welke antwoorden het besluit gebruikte."></nldd-title>
        <WaaromLijst v-if="waarom" :waarom="waarom" :teksten="vormen?.teksten" :cel-naam="naamVanCel" />
        <nldd-inline-dialog v-else-if="waaromFout" icon="warning" text="Het waarom laadt niet" :supporting-text="waaromFout"></nldd-inline-dialog>
        <nldd-activity-indicator v-else></nldd-activity-indicator>
      </nldd-container>

      <nldd-container v-if="tegen.length" gap="8">
        <nldd-title heading-level="2" size="display-6" text="Het niet eens met dit besluit?"></nldd-title>
        <nldd-button-group orientation="horizontal">
          <nldd-button
            v-for="i in tegen"
            :key="i.artikel"
            appearance="secondary"
            :text="`${soortNaam(i)} indienen`"
            @click="start(i)"
          ></nldd-button>
        </nldd-button-group>
      </nldd-container>
    </nldd-container>
  </nldd-simple-section>

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
            <SpecFormulier :velden="velden" :waarden="waarden" @zet="(n, v) => (waarden[n] = v)" />
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
