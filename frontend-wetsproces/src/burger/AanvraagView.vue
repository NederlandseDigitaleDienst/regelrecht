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
// hides stays off the form.
import { computed, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { longDate } from '../gedeeld/datum';
import { aanvraagformulier, aanvraagVelden, beginwaarden } from '../gedeeld/formulier';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import { burgerApi } from './api';
import { useBurger } from './useBurger';

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ sluit: [] }>();

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
const bezig = ref(false);
const melding = ref('');
// One key per filled-in form: a click sent twice records one application.
let sleutel = nieuweSleutel();

// What this application is about, in a sentence: the thing the case is named
// after, and the cell it is asked from. The title names a thing you apply for
// ("Gehandicaptenparkeerkaart"), so it takes an article here; a case whose
// title does not take "een" needs its own words. An application about a grant
// says it in the case's own words (`overToekenning.uitleg`), so it gets none.
const inleiding = computed(() => {
  const c = casus.value;
  if (!c?.titel || overToekenning.value) return '';
  return (
    `U vraagt een ${c.titel.toLowerCase()} aan bij ${c.cel_naam}. Hieronder staan de gegevens die de wet voor deze ` +
    'beslissing nodig heeft; bij elke vraag staat op welk artikel zij berust. Wat al bekend is, hoeft u niet in te vullen.'
  );
});
const vorm = computed(() => (artikel.value && vormen.value ? vormen.value.indieningen[artikel.value] : undefined));
const formulier = computed(() =>
  vorm.value
    ? aanvraagformulier(vorm.value, vormen.value?.teksten, wie.value?.identiteit ?? null, vormen.value?.volgorde)
    : null,
);

/** The fields of the form in their groups; what the system fills tells the applicant it needs nothing. */
const velden = computed(() =>
  (formulier.value?.groepen ?? []).flatMap((g) =>
    g.velden.map((v) => (v.systeem ? { ...v, systeem: { ...v.systeem, herkomst: `${v.systeem.herkomst} U hoeft dit niet in te vullen.` } } : v)),
  ),
);

// The sheet stays in the DOM so it animates; it loads the form when it opens,
// and an application left half-filled does not come back on the next open.
watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    melding.value = '';
    sleutel = nieuweSleutel();
    await laad();
    waarden.value = formulier.value ? beginwaarden(formulier.value.invullen) : {};
  },
  { immediate: true },
);

async function indienen() {
  if (!vorm.value || !casus.value) return;
  if (overToekenning.value && !op.value) {
    melding.value = 'Begin bij het besluit in uw dossier: deze aanvraag gaat over een besluit.';
    return;
  }
  bezig.value = true;
  melding.value = '';
  try {
    await burgerApi.indienen(
      { artikel: artikel.value, ...(overToekenning.value && op.value ? { op: op.value } : {}), velden: aanvraagVelden(vorm.value, waarden.value) },
      sleutel,
    );
    sleutel = nieuweSleutel();
    await router.push({ name: 'dossier', query: { ingediend: vorm.value.soort } });
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
      <nldd-page>
        <nldd-top-title-bar slot="header" heading-level="2" text="Nieuwe aanvraag" dismiss-text="Sluit"></nldd-top-title-bar>
        <nldd-simple-section>
          <nldd-container gap="24">
            <nldd-text v-if="inleiding"><p>{{ inleiding }}</p></nldd-text>
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
              <SpecFormulier :velden="velden" :waarden="waarden" @zet="(n, v) => (waarden[n] = v)" />
              <nldd-text v-if="casus"><p>Dagtekening: {{ longDate(casus.vandaag) }}, de dag van indiening volgens de klok van het systeem.</p></nldd-text>
              <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="De aanvraag is niet ingediend" :supporting-text="melding"></nldd-inline-dialog>
              <nldd-button-group orientation="horizontal">
                <nldd-button appearance="primary" text="Dien aanvraag in" :loading="bezig || undefined" @click="indienen"></nldd-button>
              </nldd-button-group>
            </template>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
