<script setup lang="ts">
// The work queue of the executor, the one cell the employee works for: the
// submissions that wait for its decision, as the server gives them
// (`GET /api/backoffice/werkvoorraad`), the grants whose acts are not all done
// (`na_toekenning` in the case file), and the facts it records itself. A
// submission waits when no decision was taken on it yet, or when a later
// record revoked its last decision with nothing in its place, such as the
// receipt of a judgment that annulled a decision on objection: then it is to
// be decided again, after that record (`na`).
//
// Laid out as the editor's library: the lists in the primary sidebar (Te doen
// first, see lijsten.ts), the rows of the chosen list beside them, and the
// submission being handled in the main pane (BehandelenPaneel.vue). The list
// is in the address (`?lijst=`), so it stays when a submission opens. Every
// employee opens a submission to read it; a decider decides on it there, and
// opens the next act after a grant from its row. What the account may not do
// does not show. What an external party does, such as the GGD's advice, comes
// in through the admin panel.

import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { hoofdletter, toon } from '@regelrecht/frontend-shared/chronolex/waarden';
import { foutTekst } from '../gedeeld/http';
import { longDate } from '../gedeeld/datum';
import { artikelNaam, omschrijving, rolVan } from '../gedeeld/keten';
import type { Gram, Handeling } from '../gedeeld/types';
import { backofficeApi } from './api';
import type { EigenVerzoek, Kroniek, Werkvoorraad } from './api';
import BehandelenPaneel from './BehandelenPaneel.vue';
import { feitNaam } from './feit';
import { LIJSTEN, lijstVan, lijstenVanIndiening, lijstenVanToekenning, oudsteEerst } from './lijsten';
import type { Lijst } from './lijsten';
import { verzoekenBij } from './verzoek';
import { medewerkerStand, vervolgFeiten } from '../gedeeld/vervolg';
import type { Vervolgstand } from '../gedeeld/types';
import FeitVastleggen from './FeitVastleggen.vue';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel?: string; id?: string }>();
const route = useRoute();
const router = useRouter();
const { casus, vormen, wie, kronieken, fout, laden, laad: laadBackoffice } = useBackoffice();

/** What waits for a decision of the executor. */
const werkvoorraad = ref<Werkvoorraad | null>(null);
/** The submission that was open when the queue last loaded. */
const geladenVoor = ref<string | undefined>();
/** The grants of the executor and how far their acts are. */
const vervolg = ref<Vervolgstand[]>([]);
const vervolgFout = ref('');
/** The requests the executor's policy made, to tell what waits on an answer. */
const verzoeken = ref<EigenVerzoek[]>([]);
const verzoekenFout = ref('');
/**
 * Load the queue, the chronicles, the grants and the requests; `stil` keeps
 * what shows while it loads, as when another submission is opened.
 */
async function laad(stil = false) {
  // What the pane may decide on is the queue as loaded for the submission
  // now open, never one loaded before it opened.
  geladenVoor.value = undefined;
  const voor = props.id;
  await laadBackoffice(stil);
  try {
    werkvoorraad.value = await backofficeApi.werkvoorraad();
    geladenVoor.value = voor;
  } catch (e) {
    werkvoorraad.value = null;
    fout.value = foutTekst(e);
  }
  try {
    vervolg.value = await backofficeApi.vervolg();
    vervolgFout.value = '';
  } catch (e) {
    vervolgFout.value = foutTekst(e);
  }
  try {
    verzoeken.value = (await backofficeApi.verzoeken()).verzoeken;
    verzoekenFout.value = '';
  } catch (e) {
    verzoeken.value = [];
    verzoekenFout.value = foutTekst(e);
  }
}
onMounted(() => laad());
// Opening a submission loads the queue again, so a decision another
// employee took in the meantime shows, and its button does not.
watch(
  () => props.id,
  (id, voor) => id && id !== voor && laad(true),
);

/** What the account may do at the executor; none when it may not read its chronicle. */
const recht = computed(() => wie.value?.cellen.find((c) => c.cel === casus.value?.cel));

/** The facts the executor records on their own; an act after a grant is recorded beside that grant. */
const handelingen = computed(() => {
  const eigen = vormen.value?.eigen;
  return recht.value?.beslissen && eigen?.cel === recht.value.cel
    ? eigen.handelingen.filter((h) => !vervolgFeiten(casus.value).has(h.feit))
    : [];
});

/** A row of a list: a submission to decide on, or a grant whose next act waits. */
type Rij =
  | { soort: 'indiening'; sleutel: string; sinds: string; lijsten: Lijst[]; tekst: string; regel: string; opnieuw: boolean }
  | {
      soort: 'toekenning';
      sleutel: string;
      sinds: string;
      lijsten: Lijst[];
      tekst: string;
      regel: string;
      knop: string;
      handeling?: Handeling;
    };

/**
 * A submission that waits for a decision, in words: when it came in, when
 * its decision was revoked the record to decide after, and whom it waits on
 * when a request about it is still open.
 */
function indieningsRij(stuk: Gram, na: string | undefined, kroniek: Kroniek): Rij {
  const c = casus.value;
  const naGram = na ? kroniek.grammen.find((g) => g.id === na) : undefined;
  const wachtOp = [...new Set(verzoekenBij(stuk, verzoeken.value).filter((v) => v.open).map((v) => v.aan_naam))];
  const regel = [
    `Ontvangen ${longDate(stuk.recorded_on)} · onder ${artikelNaam(stuk.basis)}`,
    naGram && c
      ? `Opnieuw beslissen na ${omschrijving(naGram, c, rolVan(naGram, kroniek.stand)).toLowerCase()} van ${longDate(naGram.recorded_on)}`
      : '',
    wachtOp.length ? `Wacht op ${wachtOp.join(' en ')}` : '',
  ];
  return {
    soort: 'indiening',
    sleutel: stuk.id,
    sinds: naGram?.recorded_on ?? stuk.recorded_on,
    lijsten: lijstenVanIndiening(wachtOp.length > 0),
    tekst: c ? omschrijving(stuk, c, 'indiening') : stuk.kind,
    regel: regel.filter(Boolean).join(' · '),
    opnieuw: !!na,
  };
}

/** Per grant whose acts are not all done: how far it is and the next act, as the case names it. */
function toekenningsRijen(): Rij[] {
  const na = casus.value?.na_toekenning;
  if (!na) return [];
  return vervolg.value.flatMap((s): Rij[] => {
    if (!s.volgende) return [];
    const volgende = na.handelingen.find((h) => h.feit === s.volgende);
    // An act the system takes itself has no button: it waits for the system.
    const doorSysteem = !!volgende?.door_het_systeem;
    // By name where the application recorded one, else by the login field.
    const wie = s.naam || (s.aanvrager ? toon(s.aanvrager) : '');
    const knop = doorSysteem ? 'wacht op het systeem' : (volgende?.knop ?? `${feitNaam(s.volgende)} vastleggen`);
    const regel = medewerkerStand(s, na);
    const gedaan = s.stappen.flatMap((st) => (st.gedaan ? [st.gedaan] : [])).sort();
    return [
      {
        soort: 'toekenning',
        sleutel: s.toekenning,
        sinds: gedaan.at(-1) ?? s.toegekend_op ?? '',
        lijsten: lijstenVanToekenning(doorSysteem),
        tekst: [casus.value?.titel, wie ? `voor ${wie}` : ''].filter(Boolean).join(' '),
        regel: recht.value?.beslissen ? `${regel} · Volgende stap: ${knop}` : regel,
        knop,
        handeling: doorSysteem ? undefined : vormen.value?.eigen.handelingen.find((h) => h.feit === s.volgende),
      },
    ];
  });
}

/** Every row, whatever list it is in. */
const rijen = computed((): Rij[] => {
  const kroniek = kronieken.value.find((k) => k.cel === casus.value?.cel);
  const open = kroniek && werkvoorraad.value?.cel === kroniek.cel ? werkvoorraad.value.open : [];
  return [...(kroniek ? open.map((w) => indieningsRij(w.stuk, w.na, kroniek)) : []), ...toekenningsRijen()];
});
/**
 * The list the address names. When it names none, the list of the open
 * submission, so the row of what the main pane shows is in view; else Te doen.
 */
const lijst = computed((): Lijst => {
  if (route.query.lijst) return lijstVan(route.query.lijst);
  return rijen.value.find((r) => r.soort === 'indiening' && r.sleutel === props.id)?.lijsten[0] ?? 'te-doen';
});
const huidigeLijst = computed(() => LIJSTEN.find((l) => l.naam === lijst.value) ?? LIJSTEN[0]);
const aantallen = computed(
  () => Object.fromEntries(LIJSTEN.map((l) => [l.naam, rijen.value.filter((r) => r.lijsten.includes(l.naam)).length])) as Record<Lijst, number>,
);
/** The lists the sidebar offers: Te doen always, another when it holds something or is open. */
const lijsten = computed(() => LIJSTEN.filter((l) => l.naam === 'te-doen' || l.naam === lijst.value || aantallen.value[l.naam] > 0));
/** The rows of the open list, what waited longest first. */
const zichtbaar = computed(() => oudsteEerst(rijen.value.filter((r) => r.lijsten.includes(lijst.value))));

/** What an empty list says: for Te doen, that the work is done and what still waits on someone else. */
const leeg = computed(() => {
  if (lijst.value !== 'te-doen') return huidigeLijst.value.leeg;
  const wacht = aantallen.value.wacht;
  return {
    tekst: 'U bent helemaal klaar',
    uitleg:
      wacht === 0
        ? 'Er ligt geen stap bij u. Een nieuwe aanvraag komt hier te staan.'
        : `Er ligt geen stap bij u. ${wacht === 1 ? 'Eén aanvraag wacht' : `${wacht} aanvragen wachten`} op een andere partij of op het systeem.`,
  };
});

/** Open the list `naam`; the open submission stays only when it is in that list, so the main pane shows a row of it. */
function kies(naam: Lijst) {
  const blijft = rijen.value.some((r) => r.soort === 'indiening' && r.sleutel === props.id && r.lijsten.includes(naam));
  router.push(blijft ? { query: { ...route.query, lijst: naam } } : { path: '/backoffice', query: { lijst: naam } });
}

function openIndiening(id: string) {
  if (!casus.value) return;
  router.push({ name: 'behandelen', params: { cel: casus.value.cel, id }, query: route.query.lijst ? { lijst: lijst.value } : {} });
}

// The back button of the list leads to the lists, that of the submission to its list.
const lijstPagina = ref<HTMLElement | null>(null);
function terug(e: Event) {
  if (lijstPagina.value && e.composedPath().includes(lijstPagina.value)) router.push({ path: '/backoffice' });
  else router.push({ path: '/backoffice', query: { lijst: lijst.value } });
}

// The fact the sheet records, in the cell `cel`.
const keuze = ref<{ cel: string; handeling: Handeling; verwijst_naar?: string; titel?: string } | null>(null);
const vastgelegd = ref('');

async function klaar(naam: string) {
  vastgelegd.value = naam;
  keuze.value = null;
  await laad(true);
}
</script>

<template>
  <nldd-navigation-split-view @back="terug">
    <nldd-page slot="primary-sidebar" sticky-header accessible-label="Aanvragen">
      <nldd-top-title-bar slot="header" text="Aanvragen" heading-level="3" collapse-anchor="werkvoorraad-titel"></nldd-top-title-bar>
      <nldd-simple-section width="full">
        <nldd-title id="werkvoorraad-titel" size="display-3"><h3>Aanvragen</h3></nldd-title>
        <nldd-text v-if="casus" color="secondary"><p>{{ hoofdletter(casus.cel_naam) }}</p></nldd-text>
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-activity-indicator v-if="laden" text="De aanvragen laden" show-text></nldd-activity-indicator>
        <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De aanvragen laden niet" :supporting-text="fout"></nldd-inline-dialog>
        <template v-else-if="casus">
          <nldd-inline-dialog v-if="!recht" icon="info" text="Geen aanvragen" supporting-text="Dit account mag geen kroniek lezen."></nldd-inline-dialog>
          <template v-else>
            <nldd-list appearance="simple" accessible-label="Lijsten">
              <nldd-list-item
                v-for="l in lijsten"
                :key="l.naam"
                size="md"
                button
                :current="l.naam === lijst || undefined"
                @click="kies(l.naam)"
              >
                <nldd-icon-cell size="20"><nldd-icon :icon="l.icoon"></nldd-icon></nldd-icon-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell :text="l.tekst"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell :text="String(aantallen[l.naam])" color="secondary" width="fit-content" horizontal-alignment="right"></nldd-text-cell>
                <nldd-spacer-cell size="2"></nldd-spacer-cell>
                <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
            <template v-if="handelingen.length">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-button-group>
                <nldd-button
                  v-for="h in handelingen"
                  :key="h.feit"
                  appearance="secondary"
                  start-icon="add"
                  :text="`${feitNaam(h.feit)} vastleggen`"
                  @click="keuze = { cel: recht.cel, handeling: h }"
                ></nldd-button>
              </nldd-button-group>
            </template>
          </template>
        </template>
      </nldd-simple-section>
    </nldd-page>

    <nldd-page
      v-if="!laden && !fout && casus && recht"
      ref="lijstPagina"
      slot="secondary-sidebar"
      sticky-header
      :accessible-label="huidigeLijst.tekst"
      :empty="route.query.lijst ? undefined : true"
    >
      <nldd-top-title-bar slot="header" :text="huidigeLijst.tekst" heading-level="3" back-text="Aanvragen" collapse-anchor="lijst-titel"></nldd-top-title-bar>
      <nldd-simple-section width="full">
        <nldd-title id="lijst-titel" size="display-3"><h3>{{ huidigeLijst.tekst }}</h3></nldd-title>
        <nldd-text v-if="zichtbaar.length" size="sm" color="secondary"><p>{{ huidigeLijst.uitleg }}</p></nldd-text>
        <nldd-spacer size="16"></nldd-spacer>
        <template v-if="vastgelegd">
          <nldd-banner variant="success" :text="`${vastgelegd} is vastgelegd`"></nldd-banner>
          <nldd-spacer size="16"></nldd-spacer>
        </template>
        <template v-if="vervolgFout">
          <nldd-inline-dialog icon="warning" icon-color="critical" text="De toekenningen laden niet" :supporting-text="vervolgFout"></nldd-inline-dialog>
          <nldd-spacer size="16"></nldd-spacer>
        </template>
        <template v-if="verzoekenFout">
          <nldd-inline-dialog icon="warning" icon-color="critical" text="Niet bekend wat op een antwoord wacht" :supporting-text="verzoekenFout"></nldd-inline-dialog>
          <nldd-spacer size="16"></nldd-spacer>
        </template>
        <nldd-list v-if="zichtbaar.length" appearance="simple" :accessible-label="huidigeLijst.tekst">
          <template v-for="r in zichtbaar" :key="r.sleutel">
            <nldd-list-item v-if="r.soort === 'indiening'" size="md" button :current="r.sleutel === id || undefined" @click="openIndiening(r.sleutel)">
              <nldd-text-cell :text="r.tekst" :supporting-text="r.regel"></nldd-text-cell>
              <nldd-cell v-if="r.opnieuw">
                <nldd-badge text="Opnieuw beslissen" color="warning" size="sm"></nldd-badge>
              </nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
            </nldd-list-item>
            <!-- For a decider the row of a grant opens the sheet of its next act; a reader only sees how far it is. -->
            <nldd-list-item
              v-else
              size="md"
              :button="(recht.beslissen && r.handeling) || undefined"
              @click="recht.beslissen && r.handeling && (keuze = { cel: recht.cel, handeling: r.handeling, verwijst_naar: r.sleutel, titel: r.knop })"
            >
              <nldd-text-cell :text="r.tekst" :supporting-text="r.regel"></nldd-text-cell>
              <template v-if="recht.beslissen && r.handeling">
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
              </template>
            </nldd-list-item>
          </template>
        </nldd-list>
        <nldd-inline-dialog v-else :text="leeg.tekst" :supporting-text="leeg.uitleg">
          <nldd-button
            v-if="lijst === 'te-doen' && aantallen.wacht > 0"
            slot="actions"
            appearance="secondary"
            text="Bekijk Wacht op anderen"
            @click="kies('wacht')"
          ></nldd-button>
        </nldd-inline-dialog>
        <template v-if="!recht.beslissen && zichtbaar.some((r) => r.soort === 'indiening')">
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-text size="sm" color="secondary">
            <p>Een beslisser neemt het besluit; u ziet de indieningen wel.</p>
          </nldd-text>
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
        :open="werkvoorraad && werkvoorraad.cel === cel && geladenVoor === id ? werkvoorraad.open : undefined"
        :terug="huidigeLijst.tekst"
        @besloten="laad(true)"
      />
      <nldd-page v-else landmarks="page">
        <nldd-simple-section width="full">
          <nldd-inline-dialog text="Kies een indiening" supporting-text="Een indiening opent hier, naast de lijst."></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
    </div>
  </nldd-navigation-split-view>

  <FeitVastleggen :keuze="keuze" :teksten="vormen?.teksten" :volgorde="vormen?.volgorde" :vastleggen="backofficeApi.vastleggen" @sluit="keuze = null" @vastgelegd="klaar" />
</template>
