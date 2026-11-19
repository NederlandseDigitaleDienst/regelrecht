<script setup lang="ts">
// One submission, for the decider of the cell that holds it: what was
// submitted, the findings the deciding article asks of the authority
// (`GET /api/formulier`, `besluiten`), a preview of what the decision would
// come to, and the decision itself. The preview may ask more: a finding a
// policy leaves to the authority for this case (`stil`). Once a decision is
// taken it shows with why it came out so.
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { schermveld } from '../gedeeld/formulier';
import type { Schermveld } from '../gedeeld/formulier';
import Gegevens from '../gedeeld/Gegevens.vue';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { besluitenOp, celNaam, omschrijving, toegewezen } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Gram, Veld, Voorvertoning, Waarom } from '../gedeeld/types';
import WaaromLijst from '../gedeeld/WaaromLijst.vue';
import { backofficeApi } from './api';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel: string; id: string }>();
const router = useRouter();
const { casus, vormen, wie, kronieken, fout, laden, laad } = useBackoffice();

const kroniek = computed(() => kronieken.value.find((k) => k.cel === props.cel));
const indiening = computed(() => kroniek.value?.grammen.find((g) => g.id === props.id) ?? null);
const mag = computed(() => wie.value?.cellen.find((c) => c.cel === props.cel)?.beslissen ?? false);
const naamVanCel = (cel: string) => (casus.value ? celNaam(cel, casus.value) : cel);

/** The decision form of the article the submission asks a decision of. */
const besluitvorm = computed(() => {
  const g = indiening.value;
  const artikel = g && vormen.value?.indieningen[g.basis]?.besluit;
  return artikel ? vormen.value?.besluiten[artikel] : undefined;
});

// The findings the preview found a policy leaves to the authority.
const stil = ref<Veld[]>([]);
const velden = computed<Schermveld[]>(() => {
  const eigen = besluitvorm.value?.bevindingen ?? [];
  const extra = stil.value.filter((s) => !eigen.some((e) => e.naam === s.naam));
  return [...eigen, ...extra].map((v) => schermveld(v, vormen.value?.teksten));
});
const bevindingen = ref<Record<string, unknown>>({});

const voorvertoning = ref<Voorvertoning | null>(null);
const genomen = ref<{ besluit: Gram; waarom: Waarom } | null>(null);
const melding = ref('');
const bezig = ref(false);
let sleutel = nieuweSleutel();

/** A decision already on the submission, taken before this screen opened. */
const eerder = computed(() =>
  indiening.value && casus.value && kroniek.value ? besluitenOp(indiening.value.id, kroniek.value.grammen, casus.value).at(-1) : undefined,
);
const eerderWaarom = ref<Waarom | null>(null);
const besluit = computed(() => genomen.value?.besluit ?? eerder.value ?? null);
const waarom = computed(() => genomen.value?.waarom ?? eerderWaarom.value);
const stand = (g: Gram) => (casus.value ? toegewezen(g, casus.value) : undefined);

async function laadEerder() {
  if (eerder.value && !genomen.value) eerderWaarom.value = await backofficeApi.waarom(eerder.value.id).catch(() => null);
}
onMounted(async () => {
  await laad();
  await laadEerder();
});

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
    <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De indiening laadt niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-inline-dialog v-else-if="!indiening || !casus" icon="info" text="Deze indiening staat niet in een kroniek die u mag lezen">
      <nldd-button slot="actions" appearance="secondary" text="Naar de werkvoorraad" @click="router.push('/backoffice')"></nldd-button>
    </nldd-inline-dialog>
    <nldd-container v-else gap="24">
      <nldd-title
        heading-level="1"
        :text="omschrijving(indiening, casus)"
        :supporting-text="`Ontvangen ${longDate(indiening.recorded_on)} door ${naamVanCel(indiening.recording_actor)} · onder ${indiening.basis}`"
      ></nldd-title>

      <nldd-container gap="8">
        <nldd-title heading-level="2" size="display-6" text="Wat er is ingediend"></nldd-title>
        <Gegevens :waarden="indiening.fields" :teksten="vormen?.teksten" label="Gegevens van de indiening" />
      </nldd-container>

      <nldd-container v-if="besluit" gap="16">
        <nldd-banner
          :variant="genomen ? 'success' : 'neutral'"
          :text="genomen ? 'Het besluit is genomen' : `Op deze indiening is besloten op ${longDate(besluit.recorded_on)}`"
        ></nldd-banner>
        <nldd-title heading-level="2" size="display-6" text="Het besluit">
          <nldd-tag
            v-if="stand(besluit) !== undefined"
            slot="end"
            :text="stand(besluit) ? 'Toegewezen' : 'Afgewezen'"
            :color="stand(besluit) ? 'success' : 'critical'"
          ></nldd-tag>
        </nldd-title>
        <Gegevens :waarden="besluit.outputs" :teksten="vormen?.teksten" label="Uitkomst van het besluit" />
        <nldd-title heading-level="3" size="display-6" text="Waarom"></nldd-title>
        <WaaromLijst v-if="waarom" :waarom="waarom" :teksten="vormen?.teksten" :cel-naam="naamVanCel" />
        <nldd-button-group orientation="horizontal">
          <nldd-button appearance="secondary" text="Terug naar de werkvoorraad" @click="router.push('/backoffice')"></nldd-button>
        </nldd-button-group>
      </nldd-container>

      <nldd-container v-else-if="mag" gap="16">
        <nldd-title
          heading-level="2"
          size="display-6"
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
          <nldd-title heading-level="3" size="display-6" text="Voorvertoning">
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
</template>
