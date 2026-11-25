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
import { useMenuactie } from '../gedeeld/schil';
import type { Gram, Handeling } from '../gedeeld/types';
import { beheerApi } from './api';
import type { OpenVerzoek, Partijen } from './api';
import BehandelenPaneel from './BehandelenPaneel.vue';
import { vastlegNaam } from './feit';
import FeitVastleggen from './FeitVastleggen.vue';
import { isFeit, useVastlegKeuze } from './vastlegadres';
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

// The parties with a server of their own (PartijView.vue there). The panel
// points to that system and enters nothing for them, until the admin says
// to enter here after all (`hierInvoeren`), for when that system does not
// work. The server offers that in test mode only (`nooduitweg`).
const hierInvoeren = ref(new Set<string>());
const gekoppeld = computed(() =>
  (panel.value?.gekoppeld ?? []).map((g) => ({
    ...g,
    nooduitweg: partijen.value.some((p) => p.cel === g.cel && p.nooduitweg),
  })),
);
/** The linked parties the panel only points to. */
const verwezen = computed(() => gekoppeld.value.filter((g) => !hierInvoeren.value.has(g.cel)));
/** The parties the admin enters for here. */
const invoer = computed(() => partijen.value.filter((p) => !p.nooduitweg || hierInvoeren.value.has(p.cel)));
function voorkantVan(cel: string): string | undefined {
  return gekoppeld.value.find((g) => g.cel === cel)?.voorkant ?? undefined;
}
function zetHierInvoeren(cel: string, aan: boolean) {
  const cellen = new Set(hierInvoeren.value);
  if (aan) cellen.add(cel);
  else cellen.delete(cel);
  hierInvoeren.value = cellen;
}

// The fact the sheet records, by the address: for a party, in answer to one of its open requests or without one.
const { keuze, kies, sluit } = useVastlegKeuze<{ cel: string; handeling: Handeling; verzoek?: string; vast?: Record<string, unknown> }>((a) => {
  const p = partijen.value.find((p) => p.cel === a.cel);
  if (!p) return null;
  if (a.bij) {
    const v = p.verzoeken.find((v) => v.verzoek.id === a.bij && isFeit(v.handeling, a));
    return v ? { cel: a.cel, handeling: v.handeling, verzoek: v.verzoek.id, vast: v.vast } : null;
  }
  const h = [...p.handelingen, ...p.zonderVerzoek].find((h) => isFeit(h, a));
  return h ? { cel: a.cel, handeling: h } : null;
});
const gedaan = ref('');

// What the admin records for a party on its own, in the bar's Vastleggen
// menu under the party's name; an answer to an open request starts at the
// request, in the panel. A party that works in a system of its own has no
// options there, unless the admin enters for it here after all.
useMenuactie(() => ({
  tekst: 'Vastleggen',
  icoon: 'add',
  opties: invoer.value.flatMap((p) => [
    ...p.handelingen.map((h) => ({ tekst: vastlegNaam(h.feit), groep: hoofdletter(p.cel_naam), kies: () => kies({ cel: p.cel, handeling: h }) })),
    ...p.zonderVerzoek.map((h) => ({
      tekst: `${vastlegNaam(h.feit)} zonder verzoek`,
      groep: hoofdletter(p.cel_naam),
      kies: () => kies({ cel: p.cel, handeling: h }),
    })),
  ]),
}));

/** What the info button beside a party says: how its work is entered here. */
function overPartij(p: { cel_naam: string; verzoeken: unknown[]; handelingen: unknown[]; zonderVerzoek: unknown[] }): string {
  return [
    p.verzoeken.length ? 'Kies een open verzoek om het antwoord vast te leggen.' : '',
    p.handelingen.length ? `Wat ${p.cel_naam} verder doet, legt u vast via Vastleggen in de balk.` : '',
    p.zonderVerzoek.length
      ? 'Via Vastleggen legt u een antwoord ook zonder verzoek vast, alleen voor een aanvrager voor wie geen verzoek openstaat, zoals iemand die eerder een kaart kreeg.'
      : '',
  ]
    .filter(Boolean)
    .join(' ');
}

async function vastgelegd(naam: string) {
  gedaan.value = `${naam} is vastgelegd`;
  sluit();
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
    <nldd-page slot="primary-sidebar" sticky-header accessible-label="Beheerpaneel">
        <nldd-top-title-bar slot="header" text="Beheerpaneel" heading-level="3" collapse-anchor="beheer-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="beheer-titel" size="display-3">
            <h3>Beheerpaneel</h3>
            <nldd-icon-button slot="end" appearance="neutral-base" size="sm" icon="info" tooltip-timing="never" text="Over het beheerpaneel">
              <nldd-popover slot="popup" role="region" accessible-label="Over het beheerpaneel">
                <nldd-container padding="16">
                  <nldd-text>
                    <p>Wat een externe partij in het proces doet, voert u hier namens haar in. De kroniek legt vast dat het via het beheerpaneel binnenkwam, en door welk account.</p>
                  </nldd-text>
                </nldd-container>
              </nldd-popover>
            </nldd-icon-button>
          </nldd-title>
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
            <nldd-inline-dialog v-if="partijen.length === 0 && gekoppeld.length === 0" text="Deze casus kent geen externe partijen"></nldd-inline-dialog>
            <template v-for="(g, i) in verwezen" :key="g.cel">
              <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
              <nldd-title size="display-5"><h4>{{ hoofdletter(g.cel_naam) }}</h4></nldd-title>
              <nldd-text color="secondary">
                <p>{{ hoofdletter(g.cel_naam) }} werkt in een eigen systeem. Verzoeken en antwoorden gaan over de koppeling; u voert hier niets voor haar in.</p>
              </nldd-text>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-button-group>
                <nldd-button
                  v-if="g.voorkant"
                  appearance="secondary"
                  end-icon="external-link"
                  :text="`Naar het systeem van ${g.cel_naam}`"
                  :href="g.voorkant"
                  target="_blank"
                ></nldd-button>
                <nldd-button
                  v-if="g.nooduitweg"
                  appearance="neutral-transparent"
                  text="Toch hier invoeren"
                  @click="zetHierInvoeren(g.cel, true)"
                ></nldd-button>
              </nldd-button-group>
            </template>
            <template v-for="(p, i) in invoer" :key="p.cel">
              <nldd-spacer v-if="i > 0 || verwezen.length" size="24"></nldd-spacer>
              <nldd-title size="display-5">
                <h4>{{ hoofdletter(p.cel_naam) }}</h4>
                <nldd-icon-button
                  v-if="overPartij(p)"
                  slot="end"
                  appearance="neutral-base"
                  size="sm"
                  icon="info"
                  tooltip-timing="never"
                  :text="`Over ${p.cel_naam}`"
                >
                  <nldd-popover slot="popup" role="region" :accessible-label="`Over ${p.cel_naam}`">
                    <nldd-container padding="16">
                      <nldd-text><p>{{ overPartij(p) }}</p></nldd-text>
                    </nldd-container>
                  </nldd-popover>
                </nldd-icon-button>
              </nldd-title>
              <template v-if="p.nooduitweg">
                <nldd-banner
                  variant="warning"
                  :text="`${hoofdletter(p.cel_naam)} werkt in een eigen systeem`"
                  supporting-text="Voer hier alleen in als dat systeem niet werkt. Wat u hier vastlegt, gaat over de koppeling naar haar toe zodra het weer bereikbaar is."
                ></nldd-banner>
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-button-group>
                  <nldd-button
                    v-if="voorkantVan(p.cel)"
                    appearance="secondary"
                    end-icon="external-link"
                    :text="`Naar het systeem van ${p.cel_naam}`"
                    :href="voorkantVan(p.cel)"
                    target="_blank"
                  ></nldd-button>
                  <nldd-button appearance="neutral-transparent" text="Invoer hier verbergen" @click="zetHierInvoeren(p.cel, false)"></nldd-button>
                </nldd-button-group>
                <nldd-spacer size="8"></nldd-spacer>
              </template>
              <template v-if="p.verzoeken.length">
                <nldd-list appearance="simple" :accessible-label="`Open verzoeken aan ${p.cel_naam}`">
                  <nldd-list-item
                    v-for="v in p.verzoeken"
                    :key="v.verzoek.id"
                    size="md"
                    button
                    @click="kies({ cel: p.cel, handeling: v.handeling, verzoek: v.verzoek.id, vast: v.vast })"
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
              <nldd-text v-if="!p.verzoeken.length && !p.open.length && !p.toe_te_zenden.length" color="secondary">
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
        <nldd-simple-section>
          <nldd-inline-dialog text="Kies wat op een partij wacht" supporting-text="Een aanvraag opent hier, naast het beheerpaneel."></nldd-inline-dialog>
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
    @sluit="sluit"
    @vastgelegd="vastgelegd"
  >
    <nldd-banner
      v-if="keuze"
      variant="neutral"
      :text="`U legt dit vast namens ${partijen.find((p) => p.cel === keuze?.cel)?.cel_naam ?? keuze.cel}`"
    ></nldd-banner>
  </FeitVastleggen>
</template>
