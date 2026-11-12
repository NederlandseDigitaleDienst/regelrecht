<script setup>
/**
 * Detail van een geregistreerde koppeling: de rechtspersoon (KvK) achter een
 * aanduiding, met organisatiemodel en moederpartij (bewerkbaar). De
 * verkiezingsuitslagen en inwoneraantallen zijn referentiedata uit
 * authentieke bronnen (Kiesraad, CBS) en zijn hier alleen te raadplegen.
 */
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import PortalHeader from '../../components/PortalHeader.vue';
import NBanner from '../../components/NBanner.vue';
import { api } from '../../api.js';
import { session } from '../../session.js';
import { b } from '../../basePad.js';

const route = useRoute();
const router = useRouter();

const navItems = [
  { text: 'Werkvoorraad', to: '/' },
  { text: 'Partijregister', to: '/partijregister' },
  { text: "Scenario's", to: '/scenarios' },
];

const ORGAAN_LABELS = {
  GEMEENTERAAD: 'Gemeenteraad',
  PROVINCIALE_STATEN: 'Provinciale Staten',
  EILANDSRAAD: 'Eilandsraad',
  WATERSCHAP: 'Waterschap',
};

// Herkomst van de referentiedata, per orgaan de verkiezing.
const ORGAAN_BRONNEN = {
  GEMEENTERAAD: 'Kiesraad, gemeenteraadsverkiezingen 2026',
  PROVINCIALE_STATEN: 'Kiesraad, provinciale statenverkiezingen 2023',
  EILANDSRAAD: 'Kiesraad, eilandsraadsverkiezingen 2023',
  WATERSCHAP: 'Kiesraad, waterschapsverkiezingen 2023',
};

const kvk = computed(() => route.params.kvk);
const partij = ref(null);
const fout = ref('');
const melding = ref('');

const aantalFormat = new Intl.NumberFormat('nl-NL');

const uitslagenPerOrgaan = computed(() => {
  if (!partij.value) return [];
  return Object.keys(ORGAAN_LABELS)
    .map((orgaan) => ({
      orgaan,
      label: ORGAAN_LABELS[orgaan],
      bron: ORGAAN_BRONNEN[orgaan],
      uitslagen: partij.value.uitslagen.filter((u) => u.orgaan === orgaan),
    }))
    .filter((groep) => groep.uitslagen.length);
});

async function laad() {
  if (!session.beoordelaar) return;
  fout.value = '';
  try {
    partij.value = await api.beheerPartij(kvk.value);
  } catch (e) {
    fout.value = e.message;
  }
}

function veld(event) {
  return event.detail?.value ?? event.target?.value ?? '';
}

// --- Koppeling bewerken ------------------------------------------------------
const bewerkSheetEl = ref(null);
const bewerkOpen = ref(false);
const bewerk = ref({ naam: '', organisatiemodel: 'CENTRAAL', moederpartij_kvk: '' });
const bewerkFout = ref('');
const bewerkBezig = ref(false);

watch(bewerkOpen, async (open) => {
  if (!open) {
    bewerkSheetEl.value?.hide();
    return;
  }
  await nextTick();
  bewerkSheetEl.value?.show();
});

function openBewerken() {
  bewerk.value = {
    naam: partij.value.naam,
    organisatiemodel: partij.value.organisatiemodel,
    moederpartij_kvk: partij.value.moederpartij_kvk ?? '',
  };
  bewerkFout.value = '';
  bewerkOpen.value = true;
}

async function bewaarGegevens() {
  bewerkFout.value = '';
  bewerkBezig.value = true;
  try {
    partij.value = await api.beheerPartijWijzigen(kvk.value, {
      naam: bewerk.value.naam.trim(),
      organisatiemodel: bewerk.value.organisatiemodel,
      moederpartij_kvk: bewerk.value.moederpartij_kvk.trim() || null,
    });
    bewerkOpen.value = false;
    melding.value = 'De koppeling is gewijzigd.';
  } catch (e) {
    bewerkFout.value = e.message;
  } finally {
    bewerkBezig.value = false;
  }
}

onMounted(laad);
watch(() => session.beoordelaar, laad);
watch(kvk, laad);
</script>

<template>
  <nldd-page>
    <PortalHeader
      slot="header"
      :items="session.beoordelaar ? navItems : []"
      portal="beoordelaar"
    />

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
        <nldd-button
          appearance="neutral-transparent"
          size="sm"
          text="Terug naar het partijregister"
          start-icon="chevron-left"
          @click="router.push('/partijregister')"
        ></nldd-button>
        <nldd-spacer size="16"></nldd-spacer>

        <NBanner v-if="fout" variant="critical" text="Er ging iets mis" :supporting-text="fout" />

        <template v-if="partij">
          <nldd-title size="display-2">
            <span slot="overline">KvK-nummer {{ partij.kvk_nummer }}</span>
            <h2>{{ partij.naam }}</h2>
            <div slot="actions">
              <nldd-button
                appearance="secondary"
                text="Koppeling bewerken"
                start-icon="pencil"
                @click="openBewerken"
              ></nldd-button>
            </div>
          </nldd-title>
          <nldd-spacer size="12"></nldd-spacer>

          <nldd-container layout="wrap" gap="8">
            <nldd-tag
              v-if="partij.status === 'ONGEKOPPELD'"
              color="warning"
              text="Nog niet gekoppeld: rechtspersoon onbekend, KvK-nummer is een placeholder"
            ></nldd-tag>
            <nldd-tag
              v-else
              :color="partij.organisatiemodel === 'CENTRAAL' ? 'accent' : 'neutral'"
              :text="partij.organisatiemodel === 'CENTRAAL' ? 'Centraal georganiseerd' : 'Decentraal georganiseerd'"
            ></nldd-tag>
            <nldd-tag
              v-if="partij.kamerzetels"
              color="success"
              :text="`${partij.kamerzetels} kamerzetels (Kiesraad, TK2025)`"
            ></nldd-tag>
            <nldd-tag
              v-if="partij.moederpartij_kvk"
              color="neutral"
              :text="`Afdeling van ${partij.moederpartij_naam ?? partij.moederpartij_kvk}`"
            ></nldd-tag>
          </nldd-container>
          <nldd-spacer size="8"></nldd-spacer>

          <template v-if="melding">
            <NBanner variant="success" :text="melding" />
            <nldd-spacer size="8"></nldd-spacer>
          </template>

          <template v-if="partij.moederpartij_kvk">
            <nldd-link
              size="sm"
              end-icon="chevron-right"
              :href="b(`/beoordelaar/partijregister/${partij.moederpartij_kvk}`)"
              :text="`Naar moederpartij ${partij.moederpartij_naam ?? partij.moederpartij_kvk}`"
              @click.prevent="router.push(`/partijregister/${partij.moederpartij_kvk}`)"
            ></nldd-link>
            <nldd-spacer size="8"></nldd-spacer>
          </template>

          <nldd-spacer size="16"></nldd-spacer>

          <nldd-title size="display-3"><h3>Verkiezingsuitslagen</h3></nldd-title>
          <nldd-spacer size="4"></nldd-spacer>
          <nldd-rich-text>
            <p>
              Referentiedata uit de officiële uitslagen van de centrale
              stembureaus; inwoneraantallen van het CBS (peildatum 1 januari).
              Deze gegevens zijn niet in dit register te wijzigen — correcties
              volgen de bron.
            </p>
          </nldd-rich-text>
          <nldd-spacer size="16"></nldd-spacer>

          <template v-if="uitslagenPerOrgaan.length">
            <template v-for="groep in uitslagenPerOrgaan" :key="groep.orgaan">
              <nldd-title size="display-4"><h4>{{ groep.label }}</h4></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-table
                columns="minmax(200px,1fr) 130px 140px 100px"
                :accessible-label="`Uitslagen ${groep.label}`"
              >
                <nldd-table-row slot="header">
                  <nldd-text-cell text="Gebied"></nldd-text-cell>
                  <nldd-text-cell text="Code"></nldd-text-cell>
                  <nldd-text-cell text="Inwoneraantal" horizontal-alignment="right"></nldd-text-cell>
                  <nldd-text-cell text="Zetels" horizontal-alignment="right"></nldd-text-cell>
                </nldd-table-row>
                <nldd-table-row
                  v-for="uitslag in groep.uitslagen"
                  :key="`${uitslag.orgaan}:${uitslag.gebied_code}`"
                >
                  <nldd-text-cell :text="uitslag.gebied_naam ?? uitslag.gebied_code"></nldd-text-cell>
                  <nldd-text-cell :text="uitslag.gebied_code" color="secondary"></nldd-text-cell>
                  <nldd-text-cell
                    :text="uitslag.inwoneraantal ? aantalFormat.format(uitslag.inwoneraantal) : '–'"
                    horizontal-alignment="right"
                  ></nldd-text-cell>
                  <nldd-text-cell
                    :text="String(uitslag.zetels)"
                    horizontal-alignment="right"
                  ></nldd-text-cell>
                </nldd-table-row>
              </nldd-table>
              <nldd-spacer size="4"></nldd-spacer>
              <nldd-rich-text>
                <p>Bron: {{ groep.bron }}.</p>
              </nldd-rich-text>
              <nldd-spacer size="20"></nldd-spacer>
            </template>
          </template>
          <nldd-inline-dialog
            v-else
            icon="inbox"
            text="Geen decentrale uitslagen"
            supporting-text="In de officiële uitslagen van de Kiesraad staat voor deze aanduiding geen zetel in een gemeenteraad, provinciale staten, eilandsraad of waterschap."
          ></nldd-inline-dialog>
        </template>
        <nldd-activity-indicator v-else-if="!fout" show-text text="Partij laden"></nldd-activity-indicator>
      </nldd-simple-section>

      <!-- Koppeling bewerken -->
      <nldd-sheet
        ref="bewerkSheetEl"
        placement="right"
        width="480px"
        accessible-label="Koppeling bewerken"
        @close="bewerkOpen = false"
      >
        <nldd-container padding="24" gap="16">
          <nldd-title size="display-3">
            <span slot="overline">KvK-nummer {{ kvk }}</span>
            <h3>Koppeling bewerken</h3>
          </nldd-title>
          <nldd-form novalidate @submit.prevent="bewaarGegevens">
            <nldd-form-field label="Geregistreerde aanduiding">
              <nldd-text-field
                :value="bewerk.naam"
                name="naam"
                @input="bewerk.naam = veld($event)"
              ></nldd-text-field>
            </nldd-form-field>
            <nldd-form-field label="Organisatiemodel">
              <nldd-dropdown>
                <select
                  :value="bewerk.organisatiemodel"
                  @change="bewerk.organisatiemodel = veld($event)"
                >
                  <option value="CENTRAAL">Centraal (afdelingen onder één KvK)</option>
                  <option value="DECENTRAAL">Decentraal (afdelingen als eigen rechtspersoon)</option>
                </select>
              </nldd-dropdown>
              <nldd-form-field-help-text>
                Bij een centraal model ontstaan afdelingen vanzelf uit de
                verkiezingsuitslagen; afdelingsbestuurders loggen in met het
                KvK-nummer van de partij en een beperkte machtiging.
              </nldd-form-field-help-text>
            </nldd-form-field>
            <nldd-form-field label="Moederpartij (optioneel)">
              <nldd-text-field
                :value="bewerk.moederpartij_kvk"
                name="moederpartij_kvk"
                placeholder="KvK-nummer van de moederpartij"
                @input="bewerk.moederpartij_kvk = veld($event)"
              ></nldd-text-field>
              <nldd-form-field-help-text>
                Alleen voor afdelingen met een eigen rechtspersoon (decentraal
                model).
              </nldd-form-field-help-text>
            </nldd-form-field>
            <template v-if="bewerkFout">
              <NBanner variant="critical" :text="bewerkFout" />
            </template>
            <nldd-form-actions>
              <nldd-button-group orientation="horizontal">
                <nldd-button
                  appearance="primary"
                  type="submit"
                  text="Opslaan"
                  :disabled="bewerkBezig || undefined"
                ></nldd-button>
                <nldd-button
                  appearance="secondary"
                  text="Annuleren"
                  @click="bewerkOpen = false"
                ></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </nldd-form>
        </nldd-container>
      </nldd-sheet>
    </template>
  </nldd-page>
</template>
