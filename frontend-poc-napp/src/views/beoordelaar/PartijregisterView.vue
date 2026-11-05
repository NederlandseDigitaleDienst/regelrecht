<script setup>
/**
 * Partijregister-beheer: zoeken en bladeren door alle geregistreerde
 * koppelingen tussen rechtspersoon (KvK) en aanduiding. Nieuwe koppelingen
 * ontstaan via een claim bij de eerste aanvraag, niet via handmatige
 * invoer. Beoordelaar-only; de backend handhaaft dat met 403.
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import PortalHeader from '../../components/PortalHeader.vue';
import NBanner from '../../components/NBanner.vue';
import HrToetsRegels from '../../components/HrToetsRegels.vue';
import { api } from '../../api.js';
import { session } from '../../session.js';
import { datum } from '../../format.js';

const router = useRouter();

const navItems = [
  { text: 'Werkvoorraad', to: '/' },
  { text: 'Partijregister', to: '/partijregister' },
  { text: "Scenario's", to: '/scenarios' },
];

const LIMIT = 25;
const zoek = ref('');
const pagina = ref(1);
const partijen = ref([]);
const totaal = ref(0);
const laden = ref(false);
const fout = ref('');
const melding = ref('');

const totaalPaginas = computed(() => Math.max(1, Math.ceil(totaal.value / LIMIT)));

const aantalFormat = new Intl.NumberFormat('nl-NL');

async function laad() {
  if (!session.beoordelaar) return;
  laden.value = true;
  fout.value = '';
  try {
    const resultaat = await api.beheerPartijen({
      zoek: zoek.value.trim(),
      offset: (pagina.value - 1) * LIMIT,
      limit: LIMIT,
    });
    partijen.value = resultaat.partijen;
    totaal.value = resultaat.totaal;
  } catch (e) {
    fout.value = e.message;
  } finally {
    laden.value = false;
  }
}

let debounceTimer = null;
function onZoek(event) {
  zoek.value = event.detail?.value ?? event.target?.value ?? '';
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    pagina.value = 1;
    laad();
  }, 300);
}
onBeforeUnmount(() => clearTimeout(debounceTimer));

function onPagina(event) {
  pagina.value = event.detail?.page ?? pagina.value;
  laad();
}

// --- Claims: rechtspersonen die een ongekoppelde aanduiding claimen --------
const claims = ref([]);
const claimBezig = ref(false);
const claimFout = ref('');

const openClaims = computed(() => claims.value.filter((c) => c.status === 'OPEN'));

async function laadClaims() {
  if (!session.beoordelaar) return;
  try {
    claims.value = (await api.beheerClaims()).claims ?? [];
  } catch {
    claims.value = [];
  }
}

async function bevestigClaim(claim) {
  claimFout.value = '';
  claimBezig.value = true;
  try {
    await api.beheerClaimBevestigen(claim.id);
    melding.value = `Claim op '${claim.aanduiding}' bevestigd: KvK ${claim.kvk_nummer} is nu de geverifieerde rechtspersoon.`;
    await Promise.all([laadClaims(), laad()]);
  } catch (e) {
    claimFout.value = e.message;
  } finally {
    claimBezig.value = false;
  }
}

// Afwijzen met reden via een sheet (show()/hide() met watch-patroon).
const afwijsSheetEl = ref(null);
const afwijsOpen = ref(false);
const afwijsClaim = ref(null);
const afwijsReden = ref('');
const afwijsFout = ref('');

watch(afwijsOpen, async (open) => {
  if (!open) {
    afwijsSheetEl.value?.hide();
    return;
  }
  await nextTick();
  afwijsSheetEl.value?.show();
});

function openAfwijzen(claim) {
  afwijsClaim.value = claim;
  afwijsReden.value = '';
  afwijsFout.value = '';
  afwijsOpen.value = true;
}

async function wijsClaimAf() {
  afwijsFout.value = '';
  claimBezig.value = true;
  try {
    await api.beheerClaimAfwijzen(afwijsClaim.value.id, afwijsReden.value.trim());
    melding.value = `Claim op '${afwijsClaim.value.aanduiding}' afgewezen. De partij kan opnieuw claimen.`;
    afwijsOpen.value = false;
    await laadClaims();
  } catch (e) {
    afwijsFout.value = e.message;
  } finally {
    claimBezig.value = false;
  }
}

function alles() {
  laad();
  laadClaims();
}

onMounted(alles);
watch(() => session.beoordelaar, alles);
</script>

<template>
  <nldd-page>
    <PortalHeader
      slot="header"
      :items="session.beoordelaar ? navItems : []"
      portal="beoordelaar"
    />

    <!-- Niet ingelogd -->
    <template v-if="session.loaded && !session.beoordelaar">
      <nldd-simple-section width="560px">
        <NBanner
          variant="warning"
          text="Inloggen vereist"
          supporting-text="Het partijregister is alleen toegankelijk voor beoordelaars van de Napp."
        />
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-button
          appearance="primary"
          text="Naar de inlogpagina"
          start-icon="login"
          @click="router.push('/')"
        ></nldd-button>
      </nldd-simple-section>
    </template>

    <template v-else-if="session.beoordelaar">
      <nldd-simple-section>
        <nldd-title size="2">
          <span slot="overline">Beoordelingsomgeving</span>
          <h2>Partijregister</h2>
        </nldd-title>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-rich-text>
          <p>
            De koppeling tussen rechtspersoon (KvK-nummer) en geregistreerde
            aanduiding, met het organisatiemodel. Nieuwe koppelingen ontstaan
            doordat een partij haar aanduiding claimt bij haar eerste
            aanvraag. De verkiezingsuitslagen zelf zijn referentiedata van de
            Kiesraad en zijn hier alleen te raadplegen.
          </p>
        </nldd-rich-text>
        <nldd-spacer size="24"></nldd-spacer>

        <NBanner v-if="melding" variant="success" :text="melding" />
        <NBanner v-if="fout" variant="critical" text="Laden mislukt" :supporting-text="fout" />
        <nldd-spacer v-if="melding || fout" size="16"></nldd-spacer>

        <!-- Open claims: rechtspersonen die een ongekoppelde aanduiding
             claimen; bevestigen koppelt het register om naar het echte
             KvK-nummer. -->
        <template v-if="openClaims.length">
          <nldd-title size="3"><h3>Claims</h3></nldd-title>
          <nldd-spacer size="4"></nldd-spacer>
          <nldd-rich-text>
            <p>
              Deze rechtspersonen claimen een nog niet gekoppelde aanduiding
              uit de verkiezingsuitslag. Controleer de (gesimuleerde)
              Handelsregister-toets en bevestig of wijs af.
            </p>
          </nldd-rich-text>
          <nldd-spacer size="16"></nldd-spacer>
          <NBanner v-if="claimFout" variant="critical" text="Beoordelen mislukt" :supporting-text="claimFout" />
          <nldd-spacer v-if="claimFout" size="16"></nldd-spacer>
          <template v-for="claim in openClaims" :key="claim.id">
            <nldd-card :accessible-label="`Claim op ${claim.aanduiding}`">
              <nldd-container padding="24" gap="12">
                <nldd-title size="4">
                  <span slot="overline">Geclaimd door KvK {{ claim.kvk_nummer }} op {{ datum(claim.created_at) }}</span>
                  <h4>{{ claim.aanduiding }}</h4>
                </nldd-title>
                <HrToetsRegels :toets="claim.hr_toets" />
                <nldd-button-group orientation="horizontal">
                  <nldd-button
                    appearance="primary"
                    text="Bevestigen"
                    start-icon="checked"
                    :disabled="claimBezig || undefined"
                    @click="bevestigClaim(claim)"
                  ></nldd-button>
                  <nldd-button
                    appearance="secondary"
                    text="Afwijzen"
                    :disabled="claimBezig || undefined"
                    @click="openAfwijzen(claim)"
                  ></nldd-button>
                </nldd-button-group>
              </nldd-container>
            </nldd-card>
            <nldd-spacer size="16"></nldd-spacer>
          </template>
          <nldd-spacer size="16"></nldd-spacer>
        </template>

        <nldd-search-field
          :value="zoek"
          placeholder="Zoek op partijnaam of KvK-nummer"
          accessible-label="Zoek op partijnaam of KvK-nummer"
          @input="onZoek"
        ></nldd-search-field>
        <nldd-spacer size="16"></nldd-spacer>

        <template v-if="partijen.length">
          <nldd-table
            columns="minmax(240px,1fr) 110px 160px 110px 130px 110px"
            accessible-label="Geregistreerde partijen"
          >
            <nldd-table-row slot="header">
              <nldd-text-cell text="Naam"></nldd-text-cell>
              <nldd-text-cell text="KvK"></nldd-text-cell>
              <nldd-text-cell text="Organisatiemodel"></nldd-text-cell>
              <nldd-text-cell text="Kamerzetels" horizontal-alignment="right"></nldd-text-cell>
              <nldd-text-cell text="Uitslagen" horizontal-alignment="right"></nldd-text-cell>
              <nldd-text-cell text=""></nldd-text-cell>
            </nldd-table-row>
            <nldd-table-row v-for="partij in partijen" :key="partij.kvk_nummer">
              <nldd-text-cell :text="partij.naam" :query="zoek.trim()"></nldd-text-cell>
              <nldd-text-cell :text="partij.kvk_nummer" color="secondary"></nldd-text-cell>
              <nldd-cell>
                <!-- ONGEKOPPELD: rechtspersoon onbekend, het KvK-nummer is
                     een placeholder; het organisatiemodel zegt dan niets. -->
                <nldd-tag
                  v-if="partij.status === 'ONGEKOPPELD'"
                  color="warning"
                  text="Nog niet gekoppeld"
                ></nldd-tag>
                <nldd-tag
                  v-else
                  :color="partij.organisatiemodel === 'CENTRAAL' ? 'accent' : 'neutral'"
                  :text="partij.organisatiemodel === 'CENTRAAL' ? 'Centraal' : 'Decentraal'"
                ></nldd-tag>
              </nldd-cell>
              <nldd-text-cell
                :text="partij.kamerzetels ? String(partij.kamerzetels) : '–'"
                horizontal-alignment="right"
              ></nldd-text-cell>
              <nldd-text-cell
                :text="String(partij.aantal_uitslagen)"
                horizontal-alignment="right"
              ></nldd-text-cell>
              <nldd-cell horizontal-alignment="right">
                <nldd-button
                  appearance="secondary"
                  size="sm"
                  text="Bekijk"
                  end-icon="chevron-right"
                  @click="router.push(`/partijregister/${partij.kvk_nummer}`)"
                ></nldd-button>
              </nldd-cell>
            </nldd-table-row>
          </nldd-table>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-pagination
            v-if="totaalPaginas > 1"
            :current="pagina"
            :total="totaalPaginas"
            centered
            @page-change="onPagina"
          ></nldd-pagination>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-rich-text>
            <p>{{ aantalFormat.format(totaal) }} {{ totaal === 1 ? 'partij' : 'partijen' }} gevonden.</p>
          </nldd-rich-text>
        </template>
        <nldd-activity-indicator v-else-if="laden" show-text text="Register laden"></nldd-activity-indicator>
        <nldd-inline-dialog
          v-else
          icon="magnifier"
          text="Geen partijen gevonden"
          supporting-text="Pas de zoekopdracht aan. Nieuwe partijen verschijnen hier zodra hun claim bij de eerste aanvraag is bevestigd."
        ></nldd-inline-dialog>
      </nldd-simple-section>

      <!-- Claim afwijzen met reden -->
      <nldd-sheet
        ref="afwijsSheetEl"
        placement="right"
        width="480px"
        accessible-label="Claim afwijzen"
        @close="afwijsOpen = false"
      >
        <nldd-container padding="24" gap="16">
          <nldd-title size="3">
            <span slot="overline">{{ afwijsClaim?.aanduiding }} · KvK {{ afwijsClaim?.kvk_nummer }}</span>
            <h3>Claim afwijzen</h3>
          </nldd-title>
          <nldd-rich-text>
            <p>
              De partij ziet de reden in haar portaal en kan daarna opnieuw
              een aanduiding claimen.
            </p>
          </nldd-rich-text>
          <nldd-form novalidate @submit.prevent="wijsClaimAf">
            <nldd-form-field label="Reden van afwijzing">
              <nldd-text-field
                :value="afwijsReden"
                name="reden"
                placeholder="Bijvoorbeeld: statutaire naam wijkt af van de aanduiding"
                @input="afwijsReden = $event.detail?.value ?? $event.target?.value ?? ''"
              ></nldd-text-field>
            </nldd-form-field>
            <template v-if="afwijsFout">
              <NBanner variant="critical" :text="afwijsFout" />
            </template>
            <nldd-form-actions>
              <nldd-button-group orientation="horizontal">
                <nldd-button
                  appearance="primary"
                  type="submit"
                  text="Afwijzen"
                  :disabled="claimBezig || !afwijsReden.trim() || undefined"
                ></nldd-button>
                <nldd-button
                  appearance="secondary"
                  text="Annuleren"
                  @click="afwijsOpen = false"
                ></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </nldd-form>
        </nldd-container>
      </nldd-sheet>
    </template>
  </nldd-page>
</template>
