<script setup lang="ts">
// A new application under the case's own article, or, with `artikel` and
// `op` in the address, one the case offers about a grant
// (`na_toekenning.aanvragen`), such as a duplicate of a card that is lost;
// that one refers to the record that granted. The fields come from the
// law (`GET /api/formulier`); the UI spec says in what order and under which
// headings they stand, how they are named, and where each value comes from.
// What the system fills (the login, the clock, a fixed value, a rule) shows
// read-only, with the value the server will record and its origin; the
// server fills it itself, so the form does not send it. What the UI spec
// hides stays off the form. Once submitted, the sheet says so and offers to
// open the application in the dossier or to start another.
import { hoofdletter, leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';
import { computed, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { longDate } from '../gedeeld/datum';
import { aanvraagformulier, aanvraagVelden, beginwaarden, herkomsten, schermveld } from '../gedeeld/formulier';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Gram } from '../gedeeld/types';
import { burgerApi } from './api';
import { AANVRAAG, AANVRAAG_WETTEN } from './routes';
import { useBurger } from './useBurger';

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ sluit: []; ingediend: [] }>();

const router = useRouter();
const route = useRoute();
/** The application about a grant the address asks for, if the case offers it. */
const overToekenning = computed(() => {
  const artikel = route.query.artikel;
  return typeof artikel === 'string' ? casus.value?.na_toekenning?.aanvragen?.find((a) => a.artikel === artikel) : undefined;
});
const op = computed(() => (typeof route.query.op === 'string' ? route.query.op : undefined));
const artikel = computed(() => overToekenning.value?.artikel ?? casus.value?.artikel);
const { casus, vormen, wie, fout, laden, laad } = useBurger();
const waarden = ref<Record<string, unknown>>({});
// The sheet has two pages: the form, and behind it where each value comes
// from. The article a question rests on is not what you need while answering
// it, and it is worth little under three inputs of one address; it stands on
// a page of its own, which the introduction links to. Which page shows follows
// from the address, so that link is a link and the browser's back button
// returns from it.
// After submitting, the sheet shows what was submitted until it closes or
// another application begins.
const ingediend = ref<Gram | null>(null);
const pagina = computed(() => (ingediend.value ? 'ingediend' : route.hash === AANVRAAG_WETTEN ? 'herkomst' : 'formulier'));
/** What was submitted, in words: `Aanvraag`. */
const soortIngediend = computed(() => hoofdletter(leesbaar((vorm.value?.soort ?? 'aanvraag').toLowerCase())));
const bezig = ref(false);
const melding = ref('');
// One key per filled-in form: a click sent twice records one application.
let sleutel = nieuweSleutel();

// What this application is about, in a sentence: the thing the case is named
// after, and the cell it is asked from. The title names a thing you apply for
// ("Gehandicaptenparkeerkaart"), so it takes an article here; a case whose
// title does not take "een" needs its own words. An application about a grant
// says it in the case's own words (`overToekenning.uitleg`), so it gets none.
/** Every value the law asks for this application: what it is called, the
 *  article it rests on, and who fills it. The ones the system fills are in it
 *  too: those are the ones a reader wonders about most. */
const gegevens = computed(() => {
  const v = vorm.value;
  if (!v) return [];
  const uitSysteem = herkomsten(v);
  return v.velden.map((veld) => {
    const scherm = schermveld(veld, vormen.value?.teksten, true);
    return {
      naam: veld.naam,
      label: scherm.label,
      artikel: scherm.veld.basis ?? '',
      herkomst: uitSysteem[veld.naam] ?? 'U vult dit zelf in.',
    };
  });
});

// The rest of the introduction names the values the law asks, and links them
// to the page that says where each comes from.
const aanhef = computed(() => {
  const c = casus.value;
  if (!c?.titel || overToekenning.value) return '';
  // With the space before the next sentence: a template trims its own.
  return `U vraagt een ${c.titel.toLowerCase()} aan bij ${c.cel_naam}. `;
});
const vorm = computed(() => (artikel.value && vormen.value ? vormen.value.indieningen[artikel.value] : undefined));
const formulier = computed(() =>
  vorm.value
    ? aanvraagformulier(
        vorm.value,
        vormen.value?.teksten,
        wie.value?.identiteit ?? null,
        vormen.value?.volgorde,
        vormen.value?.delen,
        vormen.value?.verplicht,
      )
    : null,
);

/** The fields of the form in their groups. What the system fills shows as a list, which asks nothing. */
const velden = computed(() => (formulier.value?.groepen ?? []).flatMap((g) => g.velden));

/** An empty form, with a key of its own. */
async function begin() {
  ingediend.value = null;
  melding.value = '';
  sleutel = nieuweSleutel();
  await laad();
  waarden.value = formulier.value ? beginwaarden(formulier.value.invullen) : {};
}

// The sheet stays in the DOM so it animates; it loads the form when it opens,
// and an application left half-filled does not come back on the next open.
watch(
  () => props.open,
  (open) => open && begin(),
  { immediate: true },
);

// The day of submission is the server's when it records the application, so
// it is named once it is known, not on the form.
const bevestiging = (g: Gram) =>
  `Ingediend${casus.value ? ` bij ${casus.value.cel_naam}` : ''} op ${longDate(g.recorded_on)}. ` +
  `Dat is de dagtekening van uw ${soortIngediend.value.toLowerCase()}. ` +
  'Het besluit verschijnt in uw dossier zodra een medewerker klaar is.';

/** Open what was just submitted in the dossier, which closes the sheet. */
const bekijk = (g: Gram) => router.push({ name: 'stuk', params: { id: g.id } });
/** Another application under the case's own article, on an empty form. */
async function nogEen() {
  if (route.query.artikel || route.query.op) await router.replace({ path: route.path, hash: AANVRAAG });
  await begin();
}

async function indienen() {
  if (!vorm.value || !casus.value) return;
  if (overToekenning.value && !op.value) {
    melding.value = 'Begin bij het besluit in uw dossier: deze aanvraag gaat over een besluit.';
    return;
  }
  bezig.value = true;
  melding.value = '';
  try {
    ingediend.value = await burgerApi.indienen(
      { artikel: artikel.value, ...(overToekenning.value && op.value ? { op: op.value } : {}), velden: aanvraagVelden(vorm.value, waarden.value, vormen.value?.delen) },
      sleutel,
    );
    sleutel = nieuweSleutel();
    // The dossier beside the sheet shows the new record.
    emit('ingediend');
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet width="720px" :open="open || undefined" @close="emit('sluit')">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          heading-level="2"
          :text="pagina === 'herkomst' ? 'Waar deze gegevens vandaan komen' : 'Nieuwe aanvraag'"
          :back-text="pagina === 'herkomst' ? 'Nieuwe aanvraag' : ''"
          :back-href="pagina === 'herkomst' ? AANVRAAG : ''"
          :collapse-anchor="pagina === 'herkomst' ? 'aanvraag-wetten-titel' : pagina === 'formulier' ? 'aanvraag-titel' : undefined"
          :dismiss-text="pagina === 'ingediend' ? 'Sluit' : 'Annuleer'"
        ></nldd-top-title-bar>
        <nldd-simple-section v-if="ingediend">
          <nldd-inline-dialog
            variant="success"
            heading-level="3"
            :text="`${soortIngediend} ingediend`"
            :supporting-text="bevestiging(ingediend)"
          >
            <nldd-button slot="actions" appearance="primary" :text="`Bekijk ${soortIngediend.toLowerCase()}`" @click="bekijk(ingediend)"></nldd-button>
            <nldd-button slot="actions" appearance="secondary" :text="`Doe nog een ${soortIngediend.toLowerCase()}`" @click="nogEen"></nldd-button>
          </nldd-inline-dialog>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="pagina === 'herkomst'">
          <nldd-container gap="16">
            <nldd-title id="aanvraag-wetten-titel" size="display-2"><h2>Waar deze gegevens vandaan komen</h2></nldd-title>
            <nldd-text>
              <p>
                Elk gegeven hieronder komt uit de wet. Achter elk gegeven staat het artikel dat erom vraagt, en wie het
                invult.
              </p>
            </nldd-text>
            <nldd-list appearance="simple" accessible-label="Waar elk gegeven vandaan komt">
              <nldd-list-item v-for="g in gegevens" :key="g.naam" size="md">
                <nldd-text-cell :text="g.label" :supporting-text="g.herkomst"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell horizontal-alignment="right">
                  <nldd-text size="sm" color="secondary"><p>{{ g.artikel }}</p></nldd-text>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
        </nldd-simple-section>
        <nldd-simple-section v-else>
          <nldd-container gap="24">
            <nldd-title id="aanvraag-titel" size="display-2"><h2>Nieuwe aanvraag</h2></nldd-title>
            <nldd-text v-if="formulier">
              <p>
                {{ aanhef }}Hieronder staan de
                <nldd-link :href="AANVRAAG_WETTEN" text="gegevens die de wet voor deze beslissing nodig heeft"></nldd-link>.
                Wat al bekend is, hoeft u niet in te vullen.
              </p>
            </nldd-text>
            <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
            <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Het formulier laadt niet" :supporting-text="fout"></nldd-inline-dialog>
            <template v-else-if="formulier">
              <nldd-text v-if="overToekenning"><p>{{ overToekenning.uitleg }}</p></nldd-text>
              <nldd-inline-dialog
                v-if="overToekenning && !op"
                icon="warning"
                icon-color="critical"
                text="Het is niet duidelijk waar deze aanvraag over gaat"
                supporting-text="Begin bij het besluit in uw dossier."
              ></nldd-inline-dialog>
              <nldd-inline-dialog
                v-if="vorm?.systeem_fout"
                icon="warning"
                icon-color="critical"
                text="Niet alles wat de gemeente invult, is te tonen"
                :supporting-text="vorm.systeem_fout"
              ></nldd-inline-dialog>
              <!-- The browser judges the fields on submit: a field the UI spec
                   requires that is empty stops it, and nldd-form marks it. -->
              <nldd-form>
                <form @submit.prevent="indienen">
                  <SpecFormulier
                    :velden="velden"
                    :waarden="waarden"
                    :artikelen="false"
                    bekend-kop="We gebruiken de volgende gegevens voor uw aanvraag. Deze zijn al bij ons bekend."
                    @zet="(n, v) => (waarden[n] = v)" />
                  <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="De aanvraag is niet ingediend" :supporting-text="melding"></nldd-inline-dialog>
                  <nldd-form-actions>
                    <nldd-button-group orientation="vertical">
                      <nldd-button appearance="primary" type="submit" text="Dien aanvraag in" :loading="bezig || undefined"></nldd-button>
                    </nldd-button-group>
                  </nldd-form-actions>
                </form>
              </nldd-form>
            </template>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
