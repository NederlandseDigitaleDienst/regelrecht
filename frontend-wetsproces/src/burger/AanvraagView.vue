<script setup lang="ts">
// A new application under the case's own article. The fields come from the
// law (`GET /api/formulier`); the UI spec says in what order and under which
// headings they stand, how they are named, and where each value comes from.
// What the system fills (the login, the clock, a fixed value, a rule) shows
// read-only, with the value the server will record and its origin; the
// server fills it itself, so the form does not send it. What the UI spec
// hides stays off the form.
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { aanvraagformulier, aanvraagVelden, beginwaarden } from '../gedeeld/formulier';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import { burgerApi } from './api';
import { useBurger } from './useBurger';

const router = useRouter();
const { casus, vormen, wie, fout, laden, laad } = useBurger();
const waarden = ref<Record<string, unknown>>({});
const bezig = ref(false);
const melding = ref('');
// One key per filled-in form: a click sent twice records one application.
let sleutel = nieuweSleutel();

const titel = computed(() => `Aanvraag ${casus.value?.titel?.toLowerCase() ?? ''}`.trim());
const vorm = computed(() => (casus.value && vormen.value ? vormen.value.indieningen[casus.value.artikel] : undefined));
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

onMounted(async () => {
  await laad();
  if (formulier.value) waarden.value = beginwaarden(formulier.value.invullen);
});

async function indienen() {
  if (!vorm.value || !casus.value) return;
  bezig.value = true;
  melding.value = '';
  try {
    await burgerApi.indienen({ artikel: casus.value.artikel, velden: aanvraagVelden(vorm.value, waarden.value) }, sleutel);
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
  <nldd-page landmarks="page" sticky-header :accessible-label="titel">
    <nldd-top-title-bar slot="header" :text="titel" collapse-anchor="aanvraag-titel"></nldd-top-title-bar>
    <nldd-simple-section>
      <nldd-title id="aanvraag-titel" size="display-3"><h3>{{ titel }}</h3></nldd-title>
      <nldd-text v-if="casus" color="secondary">{{ `Bij ${casus.cel_naam}` }}</nldd-text>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
      <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Het formulier laadt niet" :supporting-text="fout"></nldd-inline-dialog>
      <nldd-container v-else-if="formulier" gap="24">
        <nldd-text color="secondary">De vragen komen uit de wet. Onder elke vraag staat het artikel waar zij vandaan komt.</nldd-text>
        <nldd-inline-dialog
          v-if="vorm?.systeem_fout"
          icon="warning"
          icon-color="critical"
          text="Niet alles wat de gemeente invult, is te tonen"
          :supporting-text="vorm.systeem_fout"
        ></nldd-inline-dialog>
        <SpecFormulier :velden="velden" :waarden="waarden" @zet="(n, v) => (waarden[n] = v)" />
        <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="De aanvraag is niet ingediend" :supporting-text="melding"></nldd-inline-dialog>
        <nldd-button-group orientation="horizontal">
          <nldd-button appearance="primary" text="Aanvraag indienen" :loading="bezig || undefined" @click="indienen"></nldd-button>
        </nldd-button-group>
      </nldd-container>
    </nldd-simple-section>
  </nldd-page>
</template>
