<script setup lang="ts">
// The admin panel: per external party of the case (the GGD that advises, the
// court that decides on an appeal) what the process allows of it now, which
// the admin enters on its behalf. Nobody works for such a party in this
// server; until there is a real link with it, this is how its acts come in.
// The panel shows of a party only what an input needs: the submissions that
// wait for its decision and its decisions not sent yet, never its chronicle.
// The facts it records and the decision forms come from the forms of the
// process (`GET /api/formulier`), as everywhere in the back office.
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { omschrijving } from '../gedeeld/keten';
import type { Gram, Handeling } from '../gedeeld/types';
import { beheerApi } from './api';
import type { Partijen } from './api';
import { feitNaam } from './feit';
import FeitVastleggen from './FeitVastleggen.vue';
import { useBackoffice } from './useBackoffice';

const router = useRouter();
const { casus, vormen, fout, laden, laad } = useBackoffice();
const panel = ref<Partijen | null>(null);

async function laadAlles() {
  const [, p] = await Promise.all([laad(), beheerApi.partijen().catch((e: unknown) => foutTekst(e))]);
  if (typeof p === 'string') fout.value = p;
  else panel.value = p;
}
onMounted(laadAlles);

/** Per external party: what waits for it, and the facts it records. */
const partijen = computed(() =>
  (panel.value?.partijen ?? []).map((p) => ({
    ...p,
    handelingen: vormen.value?.andere_cellen.find((c) => c.cel === p.cel)?.handelingen ?? [],
  })),
);

const keuze = ref<{ cel: string; handeling: Handeling } | null>(null);
const gedaan = ref('');

async function vastgelegd(naam: string) {
  gedaan.value = `${naam} is vastgelegd`;
  keuze.value = null;
  await laadAlles();
}

const melding = ref('');
const bezig = ref('');
// One key per decision until its copy is sent, so a retry after a lost answer
// sends nothing twice.
const sleutels = new Map<string, string>();
async function toezenden(besluit: Gram) {
  bezig.value = besluit.id;
  melding.value = '';
  try {
    const sleutel = sleutels.get(besluit.id) ?? nieuweSleutel();
    sleutels.set(besluit.id, sleutel);
    await beheerApi.toezenden(besluit.id, sleutel);
    sleutels.delete(besluit.id);
    gedaan.value = 'Het afschrift is toegezonden';
    await laadAlles();
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = '';
  }
}
</script>

<template>
  <nldd-simple-section>
    <nldd-title
      slot="header"
      heading-level="1"
      text="Beheerpaneel"
      supporting-text="Wat een externe partij in het proces doet, voert u hier namens haar in. De kroniek legt vast dat het via het beheerpaneel binnenkwam, en door welk account."
    ></nldd-title>
    <nldd-activity-indicator v-if="laden || (!panel && !fout)"></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Het beheerpaneel laadt niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-container v-else-if="casus && panel" gap="32">
      <nldd-banner v-if="gedaan" variant="success" :text="gedaan"></nldd-banner>
      <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="Dat lukt niet" :supporting-text="melding"></nldd-inline-dialog>
      <nldd-inline-dialog v-if="partijen.length === 0" icon="info" text="Deze casus kent geen externe partijen"></nldd-inline-dialog>
      <nldd-container v-for="p in partijen" :key="p.cel" gap="12">
        <nldd-title heading-level="2" size="display-5" :text="hoofdletter(p.cel_naam)"></nldd-title>

        <nldd-list v-if="p.open.length" appearance="box-base" :accessible-label="`Wat wacht op ${p.cel_naam}`">
          <nldd-list-item v-for="g in p.open" :key="g.id">
            <nldd-text-cell :text="omschrijving(g, casus)" :supporting-text="`Ontvangen ${longDate(g.recorded_on)} · onder ${g.basis}`"></nldd-text-cell>
            <nldd-cell>
              <nldd-button
                appearance="primary"
                size="sm"
                text="Besluit invoeren"
                @click="router.push({ name: 'beheer-behandelen', params: { cel: p.cel, id: g.id } })"
              ></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>

        <nldd-list v-if="p.toe_te_zenden.length" appearance="box-base" :accessible-label="`Besluiten van ${p.cel_naam} die nog niet zijn toegezonden`">
          <nldd-list-item v-for="g in p.toe_te_zenden" :key="g.id">
            <nldd-text-cell :text="omschrijving(g, casus)" :supporting-text="`Genomen ${longDate(g.recorded_on)} · nog niet toegezonden`"></nldd-text-cell>
            <nldd-cell>
              <nldd-button
                appearance="secondary"
                size="sm"
                start-icon="send"
                text="Afschrift toezenden"
                :loading="bezig === g.id || undefined"
                @click="toezenden(g)"
              ></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>

        <nldd-button-group v-if="p.handelingen.length" orientation="horizontal">
          <nldd-button
            v-for="h in p.handelingen"
            :key="h.feit"
            appearance="secondary"
            start-icon="add"
            :text="`${feitNaam(h.feit)} vastleggen`"
            @click="keuze = { cel: p.cel, handeling: h }"
          ></nldd-button>
        </nldd-button-group>

        <nldd-text v-if="!p.open.length && !p.toe_te_zenden.length && !p.handelingen.length" color="secondary">
          Op dit moment vraagt het proces niets van {{ p.cel_naam }}.
        </nldd-text>
      </nldd-container>
    </nldd-container>
  </nldd-simple-section>

  <FeitVastleggen :keuze="keuze" :teksten="vormen?.teksten" :vastleggen="beheerApi.vastleggen" @sluit="keuze = null" @vastgelegd="vastgelegd">
    <nldd-banner
      v-if="keuze"
      variant="neutral"
      :text="`U legt dit vast namens ${partijen.find((p) => p.cel === keuze?.cel)?.cel_naam ?? keuze.cel}`"
    ></nldd-banner>
  </FeitVastleggen>
</template>
