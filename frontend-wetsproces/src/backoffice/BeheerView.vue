<script setup lang="ts">
// The admin panel: per external party of the case (the GGD that advises, the
// court that decides on an appeal) what the process allows of it now, which
// the admin enters on its behalf. Nobody works for such a party in this
// server; until there is a real link with it, this is how its acts come in.
// The panel shows of a party only what an input needs: the submissions that
// wait for its decision, its decisions not sent yet and the requests of the
// executor it has not answered, never its chronicle. A fact that answers a
// request, such as the GGD's advice, is recorded from that request only: the
// request says for whom it is, and answered it leaves the list.
// The facts it records and the decision forms come from the forms of the
// process (`GET /api/formulier`), as everywhere in the back office. Laid out
// as the work queue: the parties in the sidebar, a submission being decided
// in the main pane beside it (BehandelenPaneel.vue).
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from '../gedeeld/datum';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { artikelNaam, omschrijving, waardeTekst } from '../gedeeld/keten';
import { kaalSchermveld } from '../gedeeld/formulier';
import type { Gram, Handeling } from '../gedeeld/types';
import { beheerApi } from './api';
import type { OpenVerzoek, Partijen } from './api';
import BehandelenPaneel from './BehandelenPaneel.vue';
import { feitNaam } from './feit';
import FeitVastleggen from './FeitVastleggen.vue';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel?: string; id?: string }>();
const router = useRouter();
const { casus, vormen, wie, kronieken, fout, laden, laad } = useBackoffice();
const panel = ref<Partijen | null>(null);

async function laadAlles() {
  const [, p] = await Promise.all([laad(), beheerApi.partijen().catch((e: unknown) => foutTekst(e))]);
  if (typeof p === 'string') fout.value = p;
  else panel.value = p;
}
onMounted(laadAlles);

/**
 * Per external party: what waits for it, the facts it records on its own,
 * and per open request the fact that answers it, with the fields the request
 * already holds (`vast`).
 */
const partijen = computed(() =>
  (panel.value?.partijen ?? []).map((p) => {
    const alle = vormen.value?.andere_cellen.find((c) => c.cel === p.cel)?.handelingen ?? [];
    const opVerzoek = panel.value?.antwoord_op_verzoek ?? [];
    return {
      ...p,
      handelingen: alle.filter((h) => !opVerzoek.includes(h.feit)),
      // An answer without a request: for an applicant with none open, such
      // as one the policy asked nothing for because they had a card before.
      zonderVerzoek: alle.filter((h) => opVerzoek.includes(h.feit)),
      verzoeken: p.verzoeken.flatMap((v) => {
        const handeling = alle.find((h) => h.feit === v.antwoordfeit);
        return handeling ? [{ ...v, handeling, vast: vastUit(v, handeling) }] : [];
      }),
    };
  }),
);

/** The open requests the fact being recorded answers, in the party that records it. */
const openVoorKeuze = computed(
  () => partijen.value.find((p) => p.cel === keuze.value?.cel)?.verzoeken.filter((v) => v.handeling.feit === keuze.value?.handeling.feit) ?? [],
);

/** The fields of the answer that the request holds, with the request's values. */
function vastUit(v: OpenVerzoek, h: Handeling): Record<string, unknown> {
  return Object.fromEntries(h.velden.filter((veld) => veld.naam in (v.verzoek.fields ?? {})).map((veld) => [veld.naam, v.verzoek.fields?.[veld.naam]]));
}

/** For whom a request is, in words: each field it fixes, by its label. */
function voorWie(vast: Record<string, unknown>): string {
  return Object.entries(vast)
    .map(([naam, waarde]) => `${kaalSchermveld(naam, vormen.value?.teksten).label} ${waardeTekst(waarde)}`)
    .join(', ');
}
const gekozen = computed(() => partijen.value.find((p) => p.cel === props.cel));

// On the server of an external party its own staff work here, not an admin
// on its behalf: the same screen, in their words.
const eigen = computed(() => wie.value?.partij ?? null);
const titel = computed(() => (eigen.value ? `Verzoeken aan ${eigen.value.cel_naam}` : 'Beheerpaneel'));
const gekoppeld = computed(() => (panel.value?.gekoppeld ?? []).map((g) => g.cel_naam).join(', '));

const keuze = ref<{ cel: string; handeling: Handeling; verzoek?: string; vast?: Record<string, unknown> } | null>(null);
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
  <nldd-navigation-split-view @back="router.push('/backoffice/beheer')">
    <nldd-page slot="primary-sidebar" sticky-header :accessible-label="titel">
        <nldd-top-title-bar slot="header" :text="titel" heading-level="3" collapse-anchor="beheer-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="beheer-titel" size="display-3"><h3>{{ titel }}</h3></nldd-title>
          <nldd-text color="secondary">
            <p v-if="eigen">Wat een gemeente van {{ eigen.cel_naam }} vraagt, komt hier binnen over de koppeling. Wat u vastlegt, gaat langs dezelfde weg terug.</p>
            <p v-else>Wat een externe partij in het proces doet, voert u hier namens haar in. De kroniek legt vast dat het via het beheerpaneel binnenkwam, en door welk account.</p>
          </nldd-text>
          <template v-if="!eigen && gekoppeld">
            <nldd-spacer size="16"></nldd-spacer>
            <nldd-banner variant="neutral" :text="`${hoofdletter(gekoppeld)} werkt in een eigen systeem. Verzoeken en antwoorden gaan over de koppeling; u voert hier niets voor haar in.`"></nldd-banner>
          </template>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-activity-indicator v-if="laden || (!panel && !fout)" text="Het beheerpaneel laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Het beheerpaneel laadt niet" :supporting-text="fout"></nldd-inline-dialog>
          <template v-else-if="casus && panel">
            <template v-if="gedaan">
              <nldd-banner variant="success" :text="gedaan"></nldd-banner>
              <nldd-spacer size="16"></nldd-spacer>
            </template>
            <template v-if="melding">
              <nldd-inline-dialog icon="warning" icon-color="critical" text="Dat lukt niet" :supporting-text="melding"></nldd-inline-dialog>
              <nldd-spacer size="16"></nldd-spacer>
            </template>
            <nldd-inline-dialog v-if="partijen.length === 0 && !gekoppeld" icon="info" text="Deze casus kent geen externe partijen"></nldd-inline-dialog>
            <template v-for="(p, i) in partijen" :key="p.cel">
              <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
              <nldd-title v-if="!eigen" size="display-5"><h4>{{ hoofdletter(p.cel_naam) }}</h4></nldd-title>
              <template v-if="p.verzoeken.length">
                <nldd-text color="secondary"><p>Open verzoeken. Kies er een om het antwoord vast te leggen.</p></nldd-text>
                <nldd-list appearance="simple" :accessible-label="`Open verzoeken aan ${p.cel_naam}`">
                  <nldd-list-item
                    v-for="v in p.verzoeken"
                    :key="v.verzoek.id"
                    size="md"
                    button
                    @click="keuze = { cel: p.cel, handeling: v.handeling, verzoek: v.verzoek.id, vast: v.vast }"
                  >
                    <nldd-text-cell
                      :text="omschrijving(v.verzoek, casus, 'vastlegging')"
                      :supporting-text="`Voor ${voorWie(v.vast)} · gevraagd op ${longDate(v.verzoek.recorded_on)}`"
                    ></nldd-text-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                  </nldd-list-item>
                </nldd-list>
              </template>
              <nldd-list v-if="p.open.length" appearance="simple" :accessible-label="`Wat wacht op ${p.cel_naam}`">
                <nldd-list-item
                  v-for="{ stuk: g } in p.open"
                  :key="g.id"
                  size="md"
                  button
                  :current="g.id === id || undefined"
                  @click="router.push({ name: 'beheer-behandelen', params: { cel: p.cel, id: g.id } })"
                >
                  <nldd-text-cell :text="omschrijving(g, casus, 'indiening')" :supporting-text="`Ontvangen ${longDate(g.recorded_on)} · onder ${artikelNaam(g.basis)}`"></nldd-text-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-list v-if="p.toe_te_zenden.length" appearance="simple" :accessible-label="`Besluiten van ${p.cel_naam} die nog niet zijn toegezonden`">
                <nldd-list-item v-for="g in p.toe_te_zenden" :key="g.id" size="md">
                  <nldd-text-cell :text="omschrijving(g, casus, 'besluit')" :supporting-text="`Genomen ${longDate(g.recorded_on)} · nog niet toegezonden`"></nldd-text-cell>
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
              <template v-if="p.handelingen.length">
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-button-group>
                  <nldd-button
                    v-for="h in p.handelingen"
                    :key="h.feit"
                    appearance="secondary"
                    start-icon="add"
                    :text="`${feitNaam(h.feit)} vastleggen`"
                    @click="keuze = { cel: p.cel, handeling: h }"
                  ></nldd-button>
                </nldd-button-group>
              </template>
              <template v-if="p.zonderVerzoek.length">
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-text color="secondary">
                  <p>Zonder verzoek, alleen voor een aanvrager voor wie geen verzoek openstaat, zoals iemand die eerder een kaart kreeg:</p>
                </nldd-text>
                <nldd-button-group>
                  <nldd-button
                    v-for="h in p.zonderVerzoek"
                    :key="h.feit"
                    appearance="neutral-transparent"
                    start-icon="add"
                    :text="`${feitNaam(h.feit)} vastleggen zonder verzoek`"
                    @click="keuze = { cel: p.cel, handeling: h }"
                  ></nldd-button>
                </nldd-button-group>
              </template>
              <nldd-text v-if="!p.verzoeken.length && !p.open.length && !p.toe_te_zenden.length && !p.handelingen.length && !p.zonderVerzoek.length" color="secondary">
                <p>
                  Op dit moment vraagt het proces niets van {{ p.cel_naam }}.
                </p>
              </nldd-text>
            </template>
          </template>
        </nldd-simple-section>
      </nldd-page>

    <div slot="main" :empty="id ? undefined : true">
      <BehandelenPaneel
        v-if="cel && id && casus"
        :key="id"
        :cel="cel"
        :id="id"
        :casus="casus"
        :vormen="vormen"
        :wie="wie"
        :kronieken="kronieken"
        namens
        :open="gekozen?.open ?? []"
        @besloten="laadAlles"
      />
      <nldd-page v-else landmarks="page">
        <nldd-simple-section width="full">
          <nldd-inline-dialog text="Kies wat op een partij wacht" supporting-text="Een indiening opent hier, naast het beheerpaneel."></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
    </div>
  </nldd-navigation-split-view>

  <FeitVastleggen
    :keuze="keuze"
    :teksten="vormen?.teksten"
    :volgorde="vormen?.volgorde"
    :open="openVoorKeuze"
    :vastleggen="beheerApi.vastleggen"
    @sluit="keuze = null"
    @vastgelegd="vastgelegd"
  >
    <nldd-banner
      v-if="keuze && !eigen"
      variant="neutral"
      :text="`U legt dit vast namens ${partijen.find((p) => p.cel === keuze?.cel)?.cel_naam ?? keuze.cel}`"
    ></nldd-banner>
  </FeitVastleggen>
</template>
