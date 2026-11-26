<script setup lang="ts">
// The screen of an external party on a server of its own (docs:
// components/regelhet-koppeling), such as the GGD that examines an applicant
// for the municipality. Its own staff work here: they see what the executor
// asks of their organization, answer it, and look back at what they
// answered. Nothing of the executor's back office is here, and nothing is
// entered on anyone's behalf; the executor's admin panel is another screen
// (BeheerView.vue).
// The requests arrive over the link and the answers go back the same way.
// The fact that answers a request, and its form, come from the forms of the
// process (`GET /api/formulier`), as everywhere in the back office.
import { computed, onMounted, ref } from 'vue';
import { longDate } from '../gedeeld/datum';
import { foutTekst } from '../gedeeld/http';
import { omschrijving, waardeTekst } from '../gedeeld/keten';
import { kaalSchermveld } from '../gedeeld/formulier';
import type { Gram, Handeling } from '../gedeeld/types';
import { beheerApi } from './api';
import type { Partijen } from './api';
import { feitNaam } from './feit';
import FeitVastleggen from './FeitVastleggen.vue';
import { isFeit, useVastlegKeuze } from './vastlegadres';
import { useBackoffice } from './useBackoffice';

const { casus, vormen, wie, fout, laden, laad } = useBackoffice();
const panel = ref<Partijen | null>(null);

async function laadAlles() {
  const [, p] = await Promise.all([laad(true), beheerApi.partijen().catch((e: unknown) => foutTekst(e))]);
  if (typeof p === 'string') fout.value = p;
  else panel.value = p;
}
onMounted(laadAlles);

/** The party this server works for, as the panel gives it: the only one it lists. */
const partij = computed(() => panel.value?.partijen.find((p) => p.cel === wie.value?.partij?.cel) ?? null);
const naam = computed(() => wie.value?.partij?.cel_naam ?? '');
const vrager = computed(() => panel.value?.uitvoerder.cel_naam ?? '');
const handelingen = computed<Handeling[]>(() => vormen.value?.andere_cellen.find((c) => c.cel === partij.value?.cel)?.handelingen ?? []);

/** The fields of the answer `h` that the request `verzoek` holds, with the request's values. */
function vastUit(verzoek: Gram, h: Handeling): Record<string, unknown> {
  return Object.fromEntries(h.velden.filter((v) => v.naam in (verzoek.fields ?? {})).map((v) => [v.naam, verzoek.fields?.[v.naam]]));
}

/** For whom a request is, in words: each field it fixes, by its label. */
function voorWie(vast: Record<string, unknown>): string {
  return Object.entries(vast)
    .map(([veld, waarde]) => `${kaalSchermveld(veld, vormen.value?.teksten).label} ${waardeTekst(waarde)}`)
    .join(', ');
}

/**
 * What a request asks, in words. The party's chronicle holds the receipt of
 * a request, whose fact is named after receiving it ("verzoek om onderzoek
 * ontvangen"); in a list of requests that last word says nothing.
 */
function verzoekNaam(verzoek: Gram): string {
  return casus.value ? omschrijving(verzoek, casus.value, 'vastlegging').replace(/\s+ontvangen$/i, '') : '';
}

/** The open requests, each with the fact that answers it and the fields it already holds. */
const open = computed(() =>
  (partij.value?.verzoeken ?? []).flatMap((v) => {
    const handeling = handelingen.value.find((h) => h.feit === v.antwoordfeit);
    return handeling ? [{ ...v, handeling, vast: vastUit(v.verzoek, handeling) }] : [];
  }),
);

/** The answered requests, latest first, each with for whom it was. */
const beantwoord = computed(() =>
  (partij.value?.beantwoord ?? []).map((b) => {
    const handeling = handelingen.value.find((h) => h.feit === `${b.antwoord.basis}#${b.antwoord.fact}`);
    return { ...b, voor: handeling ? voorWie(vastUit(b.verzoek, handeling)) : '' };
  }),
);

/** The facts that answer a request, for an answer nobody asked for. */
const zonderVerzoek = computed(() => handelingen.value.filter((h) => (panel.value?.antwoord_op_verzoek ?? []).includes(h.feit)));

// The fact the sheet records, by the address: in answer to an open request, or without one.
const { keuze, kies, sluit } = useVastlegKeuze<{ cel: string; handeling: Handeling; verzoek?: string; vast?: Record<string, unknown>; titel?: string }>((a) => {
  if (a.cel !== partij.value?.cel) return null;
  if (a.bij) {
    const v = open.value.find((v) => v.verzoek.id === a.bij && isFeit(v.handeling, a));
    return v ? { cel: a.cel, handeling: v.handeling, verzoek: v.verzoek.id, vast: v.vast } : null;
  }
  const h = handelingen.value.find((h) => isFeit(h, a));
  return h ? { cel: a.cel, handeling: h } : null;
});
const openVoorKeuze = computed(() => open.value.filter((v) => v.handeling.feit === keuze.value?.handeling.feit));
const gedaan = ref('');

async function vastgelegd(feit: string) {
  gedaan.value = `${feit} is vastgelegd en gaat naar ${vrager.value}`;
  sluit();
  await laadAlles();
}
</script>

<template>
  <nldd-page landmarks="page" sticky-header :accessible-label="`Verzoeken aan ${naam}`">
    <nldd-top-title-bar slot="header" text="Verzoeken" collapse-anchor="partij-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-title id="partij-titel" size="display-3"><h3>Verzoeken</h3></nldd-title>
      <nldd-text color="secondary">
        <p>Wat {{ vrager }} van {{ naam }} vraagt, staat hier. Uw antwoord gaat rechtstreeks naar {{ vrager }} terug.</p>
      </nldd-text>
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-activity-indicator v-if="laden || (!panel && !fout)" text="De verzoeken laden" show-text></nldd-activity-indicator>
      <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De verzoeken laden niet" :supporting-text="fout"></nldd-inline-dialog>
      <nldd-container v-else-if="casus && panel" gap="16">
        <nldd-banner v-if="gedaan" variant="success" :text="gedaan"></nldd-banner>

        <nldd-title size="display-5"><h4>Te beantwoorden</h4></nldd-title>
        <nldd-list v-if="open.length" appearance="box-base" accessible-label="Verzoeken die op een antwoord wachten">
          <nldd-list-item
            v-for="v in open"
            :key="v.verzoek.id"
            size="md"
            button
            @click="kies({ cel: partij?.cel ?? '', handeling: v.handeling, verzoek: v.verzoek.id, vast: v.vast })"
          >
            <nldd-text-cell
              :text="verzoekNaam(v.verzoek)"
              :supporting-text="`Voor ${voorWie(v.vast)} · gevraagd op ${longDate(v.verzoek.recorded_on)} door ${vrager}`"
            ></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-inline-dialog v-else icon="info" text="Er wacht niets op een antwoord" :supporting-text="`Een nieuw verzoek van ${vrager} verschijnt hier vanzelf.`"></nldd-inline-dialog>

        <template v-if="beantwoord.length">
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-title size="display-5"><h4>Beantwoord</h4></nldd-title>
          <nldd-list appearance="box-base" accessible-label="Verzoeken die zijn beantwoord">
            <nldd-list-item v-for="b in beantwoord" :key="b.verzoek.id" size="md">
              <nldd-text-cell
                :text="omschrijving(b.antwoord, casus, 'vastlegging')"
                :supporting-text="`${b.voor ? `Voor ${b.voor} · ` : ''}gevraagd op ${longDate(b.verzoek.recorded_on)} · beantwoord op ${longDate(b.antwoord.recorded_on)}`"
              ></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </template>

        <template v-if="zonderVerzoek.length">
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-title size="display-5"><h4>Zonder verzoek</h4></nldd-title>
          <nldd-text color="secondary">
            <p>Alleen voor iemand over wie geen verzoek openstaat, bijvoorbeeld omdat {{ vrager }} om een onderzoek vraagt zonder dat het beleid dat voorschrijft.</p>
          </nldd-text>
          <nldd-button-group>
            <nldd-button
              v-for="h in zonderVerzoek"
              :key="h.feit"
              appearance="secondary"
              start-icon="add"
              :text="`${feitNaam(h.feit)} vastleggen`"
              @click="kies({ cel: partij?.cel ?? '', handeling: h })"
            ></nldd-button>
          </nldd-button-group>
        </template>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>

  <FeitVastleggen
    :keuze="keuze"
    :teksten="vormen?.teksten"
    :volgorde="vormen?.volgorde"
    :open="openVoorKeuze"
    :vastleggen="beheerApi.vastleggen"
    @sluit="sluit"
    @vastgelegd="vastgelegd"
  />
</template>
