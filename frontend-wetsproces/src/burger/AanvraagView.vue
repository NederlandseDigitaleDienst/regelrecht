<script setup lang="ts">
// A new application under the case's own article. The fields come from the
// law (`GET /api/formulier`) and the UI spec: what the system fills, such as
// the citizen number from the login, shows read-only; what the UI spec hides
// stays off the form.
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

const vorm = computed(() => (casus.value && vormen.value ? vormen.value.indieningen[casus.value.artikel] : undefined));
const formulier = computed(() =>
  vorm.value ? aanvraagformulier(vorm.value, vormen.value?.teksten, wie.value?.identiteit ?? null) : null,
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
  <nldd-simple-section width="720px">
    <nldd-title slot="header" heading-level="1" :text="`Aanvraag ${casus?.titel?.toLowerCase() ?? ''}`.trim()" :supporting-text="casus ? `Bij ${casus.cel_naam}` : ''"></nldd-title>
    <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Het formulier laadt niet" :supporting-text="fout"></nldd-inline-dialog>
    <nldd-container v-else-if="formulier" gap="24">
      <nldd-text color="secondary">De vragen komen uit de wet. Onder elke vraag staat het artikel waar zij vandaan komt.</nldd-text>
      <nldd-container v-if="formulier.doorSysteem.length" gap="16">
        <nldd-form-field
          v-for="s in formulier.doorSysteem"
          :key="s.veld.naam"
          :label="s.label"
          :supporting-label="s.veld.basis"
        >
          <nldd-text-field :value="s.waarde == null ? '' : String(s.waarde)" readonly></nldd-text-field>
          <nldd-form-field-help-text>{{ s.help || (s.uitInlog ? 'Ingevuld uit uw inlog.' : 'Ingevuld door het systeem.') }}</nldd-form-field-help-text>
        </nldd-form-field>
      </nldd-container>
      <SpecFormulier :velden="formulier.invullen" :waarden="waarden" @zet="(n, v) => (waarden[n] = v)" />
      <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="De aanvraag is niet ingediend" :supporting-text="melding"></nldd-inline-dialog>
      <nldd-button-group orientation="horizontal">
        <nldd-button appearance="primary" text="Aanvraag indienen" :loading="bezig || undefined" @click="indienen"></nldd-button>
      </nldd-button-group>
    </nldd-container>
  </nldd-simple-section>
</template>
