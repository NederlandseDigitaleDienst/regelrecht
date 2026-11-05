<script setup>
/**
 * Claim block for a logged-in legal entity that is not in the partijregister.
 * Shows the searchable list of ONGEKOPPELDE aanduidingen from the election
 * result (claim form), or the status of the own claim: in behandeling bij de
 * Napp, or afgewezen with reason and the option to claim again. After the
 * beoordelaar confirms the claim the registration works by itself and this
 * block no longer renders (mijn-registratie then returns the partij).
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import NBanner from './NBanner.vue';
import HrToetsRegels from './HrToetsRegels.vue';
import { api } from '../api.js';
import { datum } from '../format.js';

const claim = ref(null);
const claimGeladen = ref(false);
const aanduidingen = ref([]);
const zoek = ref('');
const fout = ref('');
const bezig = ref(false);
// After a rejection: show the claim form again instead of the status block.
const opnieuw = ref(false);

const openClaim = computed(() => claim.value?.status === 'OPEN');
const afgewezenClaim = computed(() => claim.value?.status === 'AFGEWEZEN');
const toonFormulier = computed(
  () => claimGeladen.value && (!openClaim.value && (!afgewezenClaim.value || opnieuw.value)),
);

async function laadClaim() {
  try {
    const result = await api.mijnClaim();
    claim.value = result.claim;
  } catch {
    claim.value = null;
  } finally {
    claimGeladen.value = true;
  }
}

async function laadAanduidingen() {
  try {
    const result = await api.claimAanduidingen(zoek.value.trim());
    aanduidingen.value = result.aanduidingen ?? [];
  } catch (e) {
    fout.value = e.message;
  }
}

let zoekTimer = null;
function onZoek(event) {
  zoek.value = event.detail?.value ?? event.target?.value ?? '';
  clearTimeout(zoekTimer);
  zoekTimer = setTimeout(laadAanduidingen, 300);
}
onBeforeUnmount(() => clearTimeout(zoekTimer));

async function claimAanduiding(aanduiding) {
  fout.value = '';
  bezig.value = true;
  try {
    claim.value = await api.claimIndienen(aanduiding.doel_kvk);
    opnieuw.value = false;
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

function claimOpnieuw() {
  opnieuw.value = true;
  fout.value = '';
  laadAanduidingen();
}

onMounted(async () => {
  await laadClaim();
  if (!openClaim.value) laadAanduidingen();
});
</script>

<template>
  <!-- Claim in behandeling -->
  <template v-if="openClaim">
    <NBanner
      variant="neutral"
      :text="`Uw claim op '${claim.aanduiding}' is in behandeling bij de Napp`"
      :supporting-text="`Ingediend op ${datum(claim.created_at)}. Een beoordelaar bevestigt de koppeling na controle van de Handelsregister-toets hieronder. Tot die tijd heeft uw organisatie geen aanspraken.`"
    />
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-title size="4"><h3>Handelsregister-toets (gesimuleerd)</h3></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <HrToetsRegels :toets="claim.hr_toets" />
  </template>

  <template v-else-if="claimGeladen">
    <!-- Afgewezen claim -->
    <template v-if="afgewezenClaim">
      <NBanner
        variant="critical"
        :text="`Uw claim op '${claim.aanduiding}' is afgewezen`"
        :supporting-text="`Reden: ${claim.reden_afwijzing ?? 'geen reden opgegeven'}. U kunt opnieuw een aanduiding claimen.`"
      />
      <nldd-spacer size="16"></nldd-spacer>
      <template v-if="!opnieuw">
        <nldd-button
          appearance="secondary"
          text="Opnieuw claimen"
          start-icon="refresh"
          @click="claimOpnieuw"
        ></nldd-button>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
    </template>
    <template v-else>
      <NBanner
        variant="warning"
        text="Geen registratie gevonden"
        supporting-text="Uw organisatie staat niet in het partijregister van de Napp. U kunt een aanvraag indienen, maar zonder geregistreerde zetels zal de wet haar afwijzen."
      />
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <!-- Claim-formulier -->
    <template v-if="toonFormulier">
      <nldd-title size="4"><h3>Aanduiding claimen</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-rich-text>
        <p>
          Heeft uw partij bij de laatstgehouden verkiezingen zetels behaald?
          Koppel dan uw rechtspersoon aan de geregistreerde aanduiding. De
          Napp toetst uw KvK-inschrijving aan het Handelsregister (in deze
          demo gesimuleerd) en een beoordelaar bevestigt de koppeling.
        </p>
      </nldd-rich-text>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-search-field
        :value="zoek"
        placeholder="Zoek uw aanduiding in de verkiezingsuitslag"
        accessible-label="Zoek uw aanduiding in de verkiezingsuitslag"
        @input="onZoek"
      ></nldd-search-field>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list v-if="aanduidingen.length" appearance="box-tinted">
        <nldd-list-item v-for="a in aanduidingen" :key="a.doel_kvk" size="md">
          <nldd-title-cell
            :text="a.aanduiding"
            :supporting-text="a.uitslagen.join(' · ')"
          ></nldd-title-cell>
          <nldd-spacer-cell size="12"></nldd-spacer-cell>
          <nldd-cell width="fit-content">
            <nldd-button
              appearance="secondary"
              size="sm"
              text="Aanduiding claimen"
              :disabled="bezig || undefined"
              @click="claimAanduiding(a)"
            ></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-rich-text v-else>
        <p>
          Geen ongekoppelde aanduiding gevonden{{ zoek.trim() ? ` voor '${zoek.trim()}'` : '' }}.
          Staat uw aanduiding er niet bij, dan is zij al gekoppeld of heeft
          zij geen zetels in de uitslag.
        </p>
      </nldd-rich-text>

      <template v-if="fout">
        <nldd-spacer size="16"></nldd-spacer>
        <NBanner variant="critical" text="Claimen mislukt" :supporting-text="fout" />
      </template>
    </template>
  </template>
</template>
