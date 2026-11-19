<script setup lang="ts">
// The work queue: per cell the employee works for, the submissions on which
// no decision was taken yet, and the facts the cell records, such as an
// advice. A decider opens a submission to decide on it; a handler records
// facts. What the account may not do does not show.
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter, leesbaar, zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { beginwaarden, schermveld } from '../gedeeld/formulier';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { omschrijving, openIndieningen } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Handeling } from '../gedeeld/types';
import { backofficeApi } from './api';
import { useBackoffice } from './useBackoffice';

const router = useRouter();
const { casus, vormen, wie, kronieken, fout, laden, laad } = useBackoffice();
onMounted(laad);

/** Per cell: what the account may do there, its open submissions and the facts it records. */
const cellen = computed(() =>
  (wie.value?.cellen ?? []).map((recht) => {
    const kroniek = kronieken.value.find((k) => k.cel === recht.cel);
    const v = vormen.value;
    const handelingen =
      v?.eigen.cel === recht.cel ? v.eigen.handelingen : (v?.andere_cellen.find((c) => c.cel === recht.cel)?.handelingen ?? []);
    return {
      recht,
      open: casus.value && kroniek ? openIndieningen(kroniek.grammen, casus.value) : [],
      handelingen: recht.behandelen ? handelingen : [],
    };
  }),
);

/** The name of a fact: the last part of `<regelwerk>#<article>#<fact>`. */
const feitNaam = (feit: string) => hoofdletter(leesbaar(feit.split('#').at(-1) ?? feit));

// The fact the sheet records, in the cell `cel`.
const open = ref(false);
const keuze = ref<{ cel: string; handeling: Handeling } | null>(null);
const waarden = ref<Record<string, unknown>>({});
const melding = ref('');
const bezig = ref(false);
const vastgelegd = ref('');
let sleutel = nieuweSleutel();
const velden = computed(() => (keuze.value?.handeling.velden ?? []).map((v) => schermveld(v, vormen.value?.teksten)));

function start(cel: string, handeling: Handeling) {
  keuze.value = { cel, handeling };
  waarden.value = beginwaarden(velden.value);
  melding.value = '';
  sleutel = nieuweSleutel();
  open.value = true;
}

async function vastleggen() {
  if (!keuze.value) return;
  bezig.value = true;
  melding.value = '';
  try {
    const { cel, handeling } = keuze.value;
    await backofficeApi.vastleggen({ cel, feit: handeling.feit, velden: zonderLeeg(waarden.value) }, sleutel);
    vastgelegd.value = feitNaam(handeling.feit);
    open.value = false;
    await laad();
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" heading-level="1" text="Werkvoorraad" supporting-text="Per cel waarvoor u werkt: de indieningen zonder besluit."></nldd-title>
    <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De werkvoorraad laadt niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-container v-else-if="casus" gap="32">
      <nldd-banner v-if="vastgelegd" variant="success" :text="`${vastgelegd} is vastgelegd`"></nldd-banner>
      <nldd-inline-dialog v-if="cellen.length === 0" icon="info" text="U werkt voor geen cel" supporting-text="Dit account mag geen kroniek lezen."></nldd-inline-dialog>
      <nldd-container v-for="c in cellen" :key="c.recht.cel" gap="12">
        <nldd-title heading-level="2" size="display-5" :text="hoofdletter(c.recht.cel_naam)"></nldd-title>
        <nldd-list v-if="c.open.length" appearance="box-base" :accessible-label="`Werkvoorraad van ${c.recht.cel_naam}`">
          <nldd-list-item v-for="g in c.open" :key="g.id">
            <nldd-text-cell :text="omschrijving(g, casus)" :supporting-text="`Ontvangen ${longDate(g.recorded_on)} · onder ${g.basis}`"></nldd-text-cell>
            <nldd-cell v-if="c.recht.beslissen">
              <nldd-button
                appearance="primary"
                size="sm"
                text="Behandelen"
                @click="router.push({ name: 'behandelen', params: { cel: c.recht.cel, id: g.id } })"
              ></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-text v-else color="secondary">Geen open indieningen.</nldd-text>
        <nldd-text v-if="c.open.length && !c.recht.beslissen" size="sm" color="secondary">
          Een beslisser van deze cel neemt het besluit; u ziet de indieningen wel.
        </nldd-text>
        <nldd-button-group v-if="c.handelingen.length" orientation="horizontal">
          <nldd-button
            v-for="h in c.handelingen"
            :key="h.feit"
            appearance="secondary"
            start-icon="add"
            :text="`${feitNaam(h.feit)} vastleggen`"
            @click="start(c.recht.cel, h)"
          ></nldd-button>
        </nldd-button-group>
      </nldd-container>
    </nldd-container>
  </nldd-simple-section>

  <Teleport to="body">
    <nldd-sheet width="720px" :open="open || undefined" @close="open = false">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar heading-level="2" :text="keuze ? `${feitNaam(keuze.handeling.feit)} vastleggen` : ''" dismiss-text="Sluiten"></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section v-if="keuze">
          <nldd-container gap="16">
            <nldd-text color="secondary">De gegevens komen uit {{ keuze.handeling.feit.split('#').slice(0, 2).join(' artikel ') }}.</nldd-text>
            <SpecFormulier :velden="velden" :waarden="waarden" @zet="(n, v) => (waarden[n] = v)" />
            <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="Niet vastgelegd" :supporting-text="melding"></nldd-inline-dialog>
            <nldd-button-group orientation="horizontal">
              <nldd-button appearance="primary" text="Vastleggen" :loading="bezig || undefined" @click="vastleggen"></nldd-button>
              <nldd-button appearance="neutral-transparent" text="Annuleren" @click="open = false"></nldd-button>
            </nldd-button-group>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
