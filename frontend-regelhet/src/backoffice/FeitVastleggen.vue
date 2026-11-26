<script setup lang="ts">
// The sheet in which a fact is recorded: the fields the article that requires
// it lists, as the UI spec names them, and one call to record it. The work
// queue records a fact of the executor with it, the admin panel one of an
// external party; the caller passes the call (`vastleggen`). A fact about an
// earlier record, such as an act after a grant, carries that record
// (`verwijst_naar`). A field the UI spec has the system fill shows
// read-only with the value the server records; the sheet does not send it.
// A fact that answers a request comes with that request (`verzoek`): the
// fields the request already holds, such as for whom it is, show read-only
// too, and the server takes them from the request. A fact recorded without a
// request while one is open for it (`open`) is stopped here, with a word on
// where to record it instead: the server refuses it anyway. The articles the
// fields rest on stand on a page of their own in the sheet (`herkomst`), per
// article, with who fills each field; its address is the sheet's with
// `herkomst` (vastlegadres.ts), so the back button leads back to the form.
import { computed, ref, useId, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { kleineLetter, zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { beginwaarden, geordend, schermveld } from '../gedeeld/formulier';
import type { Formulierveld, Schermveld } from '../gedeeld/formulier';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Gram, Handeling, Vormen } from '../gedeeld/types';
import type { Vastlegging } from './api';
import { feitNaam } from './feit';
import { leesVastlegHash, vastlegHash } from './vastlegadres';

const props = defineProps<{
  /**
   * The fact to record and the cell that records it, the record it is about
   * if any, and the sheet's title if not the fact's name; none closes the
   * sheet. With `verzoek`, the fact answers that request, which holds the
   * fields `vast`.
   */
  keuze: {
    cel: string;
    handeling: Handeling;
    verwijst_naar?: string;
    titel?: string;
    verzoek?: string;
    vast?: Record<string, unknown>;
  } | null;
  teksten?: Vormen['teksten'];
  /** The open requests this fact answers, by the fields each holds. */
  open?: { vast: Record<string, unknown> }[];
  /** The order of the UI spec, for the fields of the fact. */
  volgorde?: string[];
  vastleggen: (v: Vastlegging, sleutel: string) => Promise<Gram>;
}>();
const emit = defineEmits<{ sluit: []; vastgelegd: [naam: string] }>();

const waarden = ref<Record<string, unknown>>({});
const melding = ref('');
const bezig = ref(false);
let sleutel = nieuweSleutel();
const systeem = computed(() => props.keuze?.handeling.systeem ?? {});
/** The fields the request already holds. */
const uitVerzoek = computed(() => props.keuze?.vast ?? {});
const alle = computed(() => (props.keuze?.handeling.velden ?? []).map((v) => schermveld(v, props.teksten)));
/** The fields the employee fills. */
const velden = computed(() => alle.value.filter((v) => !(v.veld.naam in systeem.value) && !(v.veld.naam in uitVerzoek.value)));
/**
 * Every field of the fact for the form; one the system fills or the request
 * holds carries its value and where it comes from (`systeem`), and shows
 * read-only in its place in its group.
 */
// One flat list without group headings: the groups of the UI spec are those
// of the applicant's form ("Uw persoonsgegevens"), which an employee reads as
// about themselves.
const formvelden = computed<Formulierveld[]>(() =>
  alle.value.map((s: Schermveld) => {
    const v = { ...s, groep: '' };
    const naam = v.veld.naam;
    if (naam in uitVerzoek.value) return { ...v, systeem: { waarde: uitVerzoek.value[naam], herkomst: 'Uit het verzoek.' } };
    const hoe = systeem.value[naam];
    if (!hoe) return v;
    return {
      ...v,
      systeem: {
        waarde: hoe.waarde,
        herkomst: hoe.soort === 'datum' ? 'De dag van vastleggen, volgens de klok van het systeem.' : 'Vult het systeem in.',
      },
    };
  }),
);
/** The fields per article they rest on, in the order of the form and of the first field of each, with who fills each. */
const herkomstPerArtikel = computed(() => {
  const per = new Map<string, { naam: string; tekst: string }[]>();
  for (const v of geordend(formvelden.value, props.volgorde).flatMap((g) => g.velden)) {
    const artikel = v.veld.basis || 'Zonder artikel';
    const bij = per.get(artikel) ?? [];
    per.set(artikel, bij);
    const wie = v.systeem ? kleineLetter(v.systeem.herkomst) : 'u vult dit in.';
    bij.push({ naam: v.veld.naam, tekst: `${v.label}: ${wie}` });
  }
  return [...per].map(([artikel, gegevens]) => ({ artikel, gegevens }));
});
const route = useRoute();
const router = useRouter();
const adres = computed(() =>
  props.keuze ? { cel: props.keuze.cel, feit: props.keuze.handeling.feit, bij: props.keuze.verwijst_naar ?? props.keuze.verzoek } : null,
);
const HERKOMST = computed(() => (adres.value ? vastlegHash(adres.value, true) : ''));
const HERKOMST_TITEL = 'Waar deze gegevens vandaan komen';
const knop = 'Leg vast';
const pagina = computed<'formulier' | 'herkomst'>(() => (leesVastlegHash(route.hash)?.herkomst ? 'herkomst' : 'formulier'));
const naar = (herkomst: boolean) => {
  if (adres.value) void router.push({ path: route.path, query: route.query, hash: vastlegHash(adres.value, herkomst) });
};
const titelId = `${useId()}-titel`;
const titel = computed(() => (props.keuze ? (props.keuze.titel ?? `${feitNaam(props.keuze.handeling.feit)} vastleggen`) : ''));

// Each fact the sheet opens on starts from an empty form and a new key; the
// same fact, found again after the screen reloads, keeps what was typed.
watch(
  () => (adres.value ? vastlegHash(adres.value) : ''),
  () => {
    waarden.value = beginwaarden(velden.value);
    melding.value = '';
    sleutel = nieuweSleutel();
  },
);

async function opslaan() {
  if (!props.keuze) return;
  melding.value = '';
  // The server's rule (`verzoek::open_voor`): a request is this fact's when
  // every field both hold has the same value, over the fields the form sends.
  const gezet = zonderLeeg(waarden.value);
  const zelfde = (vast: Record<string, unknown>) =>
    Object.entries(vast)
      .filter(([naam]) => naam in gezet)
      .every(([naam, w]) => gezet[naam] === w);
  if (!props.keuze.verzoek && (props.open ?? []).some((v) => zelfde(v.vast))) {
    melding.value = 'Hierom is al gevraagd, en dat verzoek staat nog open. Leg het antwoord vast vanuit dat verzoek, in de lijst open verzoeken.';
    return;
  }
  bezig.value = true;
  try {
    const { cel, handeling, verwijst_naar, verzoek } = props.keuze;
    const velden = zonderLeeg(waarden.value);
    for (const naam of [...Object.keys(systeem.value), ...Object.keys(uitVerzoek.value)]) delete velden[naam];
    await props.vastleggen(
      { cel, feit: handeling.feit, velden, ...(verwijst_naar ? { verwijst_naar } : {}), ...(verzoek ? { verzoek } : {}) },
      sleutel,
    );
    emit('vastgelegd', feitNaam(handeling.feit));
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet width="720px" :open="keuze ? true : undefined" @close="emit('sluit')">
      <nldd-page :key="`${titel}-${pagina}`" sticky-header>
        <nldd-top-title-bar
          slot="header"
          heading-level="2"
          :text="pagina === 'herkomst' ? HERKOMST_TITEL : titel"
          :back-text="pagina === 'herkomst' ? titel : ''"
          dismiss-text="Annuleer"
          :collapse-anchor="keuze ? titelId : undefined"
          @back="naar(false)"
        ></nldd-top-title-bar>
        <nldd-simple-section v-if="keuze && pagina === 'herkomst'">
          <nldd-container>
            <nldd-title :id="titelId" size="display-2" heading-level="2" :text="HERKOMST_TITEL"></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-text>
              <p>Elk gegeven komt uit de wet. Hieronder staat per artikel welke gegevens het vraagt, en wie elk gegeven invult.</p>
            </nldd-text>
            <template v-for="a in herkomstPerArtikel" :key="a.artikel">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title heading-level="3" size="display-6" :text="a.artikel"></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-rich-text>
                <ul>
                  <li v-for="g in a.gegevens" :key="g.naam">{{ g.tekst }}</li>
                </ul>
              </nldd-rich-text>
            </template>
          </nldd-container>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="keuze">
          <nldd-container>
            <nldd-title :id="titelId" size="display-2" heading-level="2" :text="titel"></nldd-title>
            <nldd-spacer size="24"></nldd-spacer>
            <template v-if="$slots.default">
              <slot></slot>
              <nldd-spacer size="16"></nldd-spacer>
            </template>
            <nldd-text>
              <p>
                Hieronder staan de
                <nldd-link><a :href="HERKOMST" @click.prevent="naar(true)">gegevens die de wet hiervoor vraagt</a></nldd-link>.
              </p>
            </nldd-text>
            <nldd-spacer size="16"></nldd-spacer>
            <SpecFormulier :velden="formvelden" :waarden="waarden" :volgorde="volgorde" :artikelen="false" @zet="(n, v) => (waarden[n] = v)" />
            <template v-if="melding">
              <nldd-spacer size="16"></nldd-spacer>
              <nldd-inline-dialog icon="warning" icon-color="critical" text="Niet vastgelegd" :supporting-text="melding"></nldd-inline-dialog>
            </template>
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-button-group orientation="vertical">
              <nldd-button appearance="primary" :text="knop" :loading="bezig || undefined" @click="opslaan"></nldd-button>
            </nldd-button-group>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
