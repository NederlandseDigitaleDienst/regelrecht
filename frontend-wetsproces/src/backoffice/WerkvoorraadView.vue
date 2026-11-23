<script setup lang="ts">
// The work queue of the executor, the one cell the employee works for: the
// submissions that wait for its decision, as the server gives them
// (`GET /api/backoffice/werkvoorraad`), and the facts it records itself. A
// submission waits when no decision was taken on it yet, or when a later
// record revoked its last decision with nothing in its place, such as the
// receipt of a judgment that annulled a decision on objection: then it is to
// be decided again, after that record (`na`). Every employee opens a
// submission to read it, and a decider decides on it there and records
// facts. What the account may not do does not show. What an external party
// does, such as the GGD's advice, comes in through the admin panel. Laid out as the
// editor's library: the queue in the sidebar, the submission being handled
// in the main pane beside it (BehandelenPaneel.vue). Under the queue, what
// follows a grant (`na_toekenning` in the case file): each grant whose acts
// are not all done, how far it is, and for a decider the next act as a
// button. Those acts are recorded only there, never on their own.
import { computed, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { hoofdletter, toon } from '@regelrecht/frontend-shared/chronolex/waarden';
import { foutTekst } from '../gedeeld/http';
import { longDate } from '../gedeeld/datum';
import { artikelNaam, omschrijving, rolVan } from '../gedeeld/keten';
import type { Gram, Handeling } from '../gedeeld/types';
import { backofficeApi } from './api';
import type { Kroniek, Werkvoorraad } from './api';
import BehandelenPaneel from './BehandelenPaneel.vue';
import { feitNaam } from './feit';
import { medewerkerStand, vervolgFeiten } from '../gedeeld/vervolg';
import type { Vervolgstand } from '../gedeeld/types';
import FeitVastleggen from './FeitVastleggen.vue';
import { useBackoffice } from './useBackoffice';

const props = defineProps<{ cel?: string; id?: string }>();
const router = useRouter();
const { casus, vormen, wie, kronieken, fout, laden, laad: laadBackoffice } = useBackoffice();

/** What waits for a decision of the executor. */
const werkvoorraad = ref<Werkvoorraad | null>(null);
/** The submission that was open when the queue last loaded. */
const geladenVoor = ref<string | undefined>();
/** The grants of the executor and how far their acts are. */
const vervolg = ref<Vervolgstand[]>([]);
const vervolgFout = ref('');
/**
 * Load the queue, the chronicles and the grants; `stil` keeps what shows
 * while it loads, as when another submission is opened.
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
}
onMounted(() => laad());
// Opening a submission loads the queue again, so a decision another
// employee took in the meantime shows, and its button does not.
watch(
  () => props.id,
  (id, voor) => id && id !== voor && laad(true),
);

/** Per grant whose acts are not all done: how far it is and the next act, as the case names it. */
const openToekenningen = computed(() => {
  const na = casus.value?.na_toekenning;
  if (!na) return [];
  return vervolg.value
    .filter((s) => s.volgende)
    .map((s) => {
      const volgende = na.handelingen.find((h) => h.feit === s.volgende);
      // An act the system takes itself has no button: it waits for the system.
      const doorSysteem = !!volgende?.door_het_systeem;
      // By name where the application recorded one, else by the login field.
      const wie = s.naam || (s.aanvrager ? toon(s.aanvrager) : '');
      return {
        stand: s,
        tekst: [casus.value?.titel, wie ? `voor ${wie}` : ''].filter(Boolean).join(' '),
        regel: medewerkerStand(s, na),
        knop: doorSysteem ? 'wacht op het systeem' : (volgende?.knop ?? `${feitNaam(s.volgende ?? '')} vastleggen`),
        handeling: doorSysteem ? undefined : vormen.value?.eigen.handelingen.find((h) => h.feit === s.volgende),
      };
    });
});

/** The executor: what the account may do there, its open submissions and the facts it records. */
const cellen = computed(() =>
  (wie.value?.cellen ?? []).map((recht) => {
    const kroniek = kronieken.value.find((k) => k.cel === recht.cel);
    const eigen = vormen.value?.eigen;
    return {
      recht,
      open: kroniek && werkvoorraad.value?.cel === recht.cel ? werkvoorraad.value.open.map((w) => rij(w.stuk, w.na, kroniek)) : [],
      // An act after a grant is recorded beside that grant, not on its own.
      handelingen:
        recht.beslissen && eigen?.cel === recht.cel
          ? eigen.handelingen.filter((h) => !vervolgFeiten(casus.value).has(h.feit))
          : [],
    };
  }),
);

/**
 * A row of the queue: the submission in words, and when its decision was
 * revoked, the record to decide after, in words.
 */
function rij(stuk: Gram, na: string | undefined, kroniek: Kroniek) {
  const c = casus.value;
  const naGram = na ? kroniek.grammen.find((g) => g.id === na) : undefined;
  const regel = `Ontvangen ${longDate(stuk.recorded_on)} · onder ${artikelNaam(stuk.basis)}`;
  return {
    stuk,
    opnieuw: !!na,
    tekst: c ? omschrijving(stuk, c, 'indiening') : stuk.kind,
    regel:
      naGram && c
        ? `${regel} · Opnieuw beslissen na ${omschrijving(naGram, c, rolVan(naGram, kroniek.stand)).toLowerCase()} van ${longDate(naGram.recorded_on)}`
        : regel,
  };
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
  <nldd-navigation-split-view @back="router.push('/backoffice')">
    <nldd-page slot="primary-sidebar" sticky-header accessible-label="Werkvoorraad">
        <nldd-top-title-bar slot="header" text="Werkvoorraad" heading-level="3" collapse-anchor="werkvoorraad-titel"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-title id="werkvoorraad-titel" size="display-3"><h3>Werkvoorraad</h3></nldd-title>
          <nldd-text color="secondary"><p>De indieningen die op een besluit wachten.</p></nldd-text>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-activity-indicator v-if="laden" text="De werkvoorraad laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="De werkvoorraad laadt niet" :supporting-text="fout"></nldd-inline-dialog>
          <template v-else-if="casus">
            <template v-if="vastgelegd">
              <nldd-banner variant="success" :text="`${vastgelegd} is vastgelegd`"></nldd-banner>
              <nldd-spacer size="16"></nldd-spacer>
            </template>
            <nldd-inline-dialog v-if="cellen.length === 0" icon="info" text="Geen werkvoorraad" supporting-text="Dit account mag geen kroniek lezen."></nldd-inline-dialog>
            <template v-for="(c, i) in cellen" :key="c.recht.cel">
              <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
              <nldd-title size="display-5"><h4>{{ hoofdletter(c.recht.cel_naam) }}</h4></nldd-title>
              <nldd-list v-if="c.open.length" appearance="simple" :accessible-label="`Werkvoorraad van ${c.recht.cel_naam}`">
                <nldd-list-item
                  v-for="r in c.open"
                  :key="r.stuk.id"
                  size="md"
                  button
                  :current="r.stuk.id === id || undefined"
                  @click="router.push({ name: 'behandelen', params: { cel: c.recht.cel, id: r.stuk.id } })"
                >
                  <nldd-text-cell :text="r.tekst" :supporting-text="r.regel"></nldd-text-cell>
                  <nldd-cell v-if="r.opnieuw">
                    <nldd-badge text="Opnieuw beslissen" color="warning" size="sm"></nldd-badge>
                  </nldd-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-text v-else color="secondary"><p>Geen open indieningen.</p></nldd-text>
              <nldd-text v-if="c.open.length && !c.recht.beslissen" size="sm" color="secondary">
                <p>Een beslisser neemt het besluit; u ziet de indieningen wel.</p>
              </nldd-text>
              <template v-if="c.handelingen.length">
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-button-group>
                  <nldd-button
                    v-for="h in c.handelingen"
                    :key="h.feit"
                    appearance="secondary"
                    start-icon="add"
                    :text="`${feitNaam(h.feit)} vastleggen`"
                    @click="keuze = { cel: c.recht.cel, handeling: h }"
                  ></nldd-button>
                </nldd-button-group>
              </template>
              <template v-if="c.recht.cel === casus.cel && casus.na_toekenning">
                <nldd-spacer size="24"></nldd-spacer>
                <nldd-title size="display-6"><h5>Na de toekenning</h5></nldd-title>
                <nldd-text color="secondary"><p>Wat er na een toekenning nog moet gebeuren, per toekenning de volgende stap.</p></nldd-text>
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-inline-dialog v-if="vervolgFout" icon="warning" icon-color="critical" text="Dit overzicht laadt niet" :supporting-text="vervolgFout"></nldd-inline-dialog>
                <nldd-list v-else-if="openToekenningen.length" appearance="simple" accessible-label="Na de toekenning">
                  <!-- For a decider the row opens the sheet of the next act; a reader only sees how far it is. -->
                  <nldd-list-item
                    v-for="t in openToekenningen"
                    :key="t.stand.toekenning"
                    size="md"
                    :button="(c.recht.beslissen && t.handeling) || undefined"
                    @click="c.recht.beslissen && t.handeling && (keuze = { cel: c.recht.cel, handeling: t.handeling, verwijst_naar: t.stand.toekenning, titel: t.knop })"
                  >
                    <nldd-text-cell :text="t.tekst" :supporting-text="c.recht.beslissen ? `${t.regel} · Volgende stap: ${t.knop}` : t.regel"></nldd-text-cell>
                    <template v-if="c.recht.beslissen && t.handeling">
                      <nldd-spacer-cell size="8"></nldd-spacer-cell>
                      <nldd-icon-cell size="20"><nldd-icon icon="chevron-right"></nldd-icon></nldd-icon-cell>
                    </template>
                  </nldd-list-item>
                </nldd-list>
                <nldd-text v-else color="secondary"><p>Er wacht niets na een toekenning.</p></nldd-text>
              </template>
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
        :open="werkvoorraad && werkvoorraad.cel === cel && geladenVoor === id ? werkvoorraad.open : undefined"
        @besloten="laad(true)"
      />
      <nldd-page v-else landmarks="page">
        <nldd-simple-section width="full">
          <nldd-inline-dialog text="Kies een indiening" supporting-text="Een indiening opent hier, naast de werkvoorraad."></nldd-inline-dialog>
        </nldd-simple-section>
      </nldd-page>
    </div>
  </nldd-navigation-split-view>

  <FeitVastleggen :keuze="keuze" :teksten="vormen?.teksten" :volgorde="vormen?.volgorde" :vastleggen="backofficeApi.vastleggen" @sluit="keuze = null" @vastgelegd="klaar" />
</template>
