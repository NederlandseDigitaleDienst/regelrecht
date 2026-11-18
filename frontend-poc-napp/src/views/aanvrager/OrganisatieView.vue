<script setup>
/**
 * Mijn organisatie: de gegevens van de rechtspersoon, los van de aanvragen.
 * Registratie (koppeling met de aanduiding, organisatiemodel) is leesbaar;
 * de rekening voor uitbetaling is hier te beheren door het tekenbevoegd
 * bestuur (één rekening per rechtspersoon, art. 27 Wpp).
 */
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import PortalHeader from '../../components/PortalHeader.vue';
import NBanner from '../../components/NBanner.vue';
import { api } from '../../api.js';
import { session } from '../../session.js';

const router = useRouter();

const navItems = [
  { text: 'Mijn aanvragen', to: '/' },
  { text: 'Nieuwe aanvraag', to: '/nieuw' },
  { text: 'Mijn organisatie', to: '/organisatie' },
];

const registratie = ref(null);

const machtiging = computed(() => session.aanvrager?.machtiging ?? null);
const beperkteMachtiging = computed(() => machtiging.value?.type === 'BEPERKT');

const ORGANISATIEMODEL_LABELS = {
  CENTRAAL: 'Centraal: afdelingen vallen onder deze rechtspersoon',
  DECENTRAAL: 'Decentraal: afdelingen zijn eigen rechtspersonen',
};

async function laadRegistratie() {
  if (!session.aanvrager) return;
  try {
    registratie.value = await api.mijnRegistratie();
  } catch {
    registratie.value = null;
  }
}

function veld(event) {
  return event.detail?.value ?? event.target?.value ?? '';
}

// --- Rekening voor uitbetaling (één rekening per rechtspersoon, art. 27) ---
const rekening = ref(null);
const rekeningSheetEl = ref(null);
const rekeningOpen = ref(false);
const rekeningForm = ref({ iban: '', tenaamstelling: '' });
const rekeningFout = ref('');
const rekeningBezig = ref(false);
const rekeningMelding = ref('');

watch(rekeningOpen, async (open) => {
  if (!open) {
    rekeningSheetEl.value?.hide();
    return;
  }
  await nextTick();
  rekeningSheetEl.value?.show();
});

function openRekening() {
  rekeningForm.value = {
    iban: rekening.value?.iban ?? '',
    tenaamstelling: rekening.value?.tenaamstelling ?? '',
  };
  rekeningFout.value = '';
  rekeningMelding.value = '';
  rekeningOpen.value = true;
}

async function bewaarRekening() {
  rekeningFout.value = '';
  rekeningBezig.value = true;
  try {
    const resultaat = await api.mijnRekeningWijzigen({
      iban: rekeningForm.value.iban.trim(),
      tenaamstelling: rekeningForm.value.tenaamstelling.trim(),
    });
    rekening.value = resultaat;
    rekeningOpen.value = false;
    const n = resultaat.geactiveerde_betaalopdrachten ?? 0;
    rekeningMelding.value =
      n > 0
        ? `Het rekeningnummer is vastgelegd. ${n === 1 ? 'Eén aangehouden betaalopdracht is' : `${n} aangehouden betaalopdrachten zijn`} alsnog klaargezet voor uitbetaling (artikel 27 Wpp).`
        : 'Het rekeningnummer voor uitbetaling is vastgelegd.';
  } catch (e) {
    rekeningFout.value = e.message;
  } finally {
    rekeningBezig.value = false;
  }
}

async function laadRekening() {
  if (!session.aanvrager) {
    rekening.value = null;
    return;
  }
  try {
    rekening.value = await api.mijnRekening();
  } catch {
    rekening.value = null;
  }
}

function laadAlles() {
  laadRegistratie();
  laadRekening();
}

onMounted(laadAlles);
watch(() => session.aanvrager, laadAlles);
</script>

<template>
  <nldd-page>
    <PortalHeader
      slot="header"
      :items="session.aanvrager ? navItems : []"
      portal="aanvrager"
    />

    <template v-if="session.loaded && !session.aanvrager">
      <nldd-simple-section width="560px">
        <nldd-inline-dialog
          variant="alert"
          text="U bent niet ingelogd"
          supporting-text="Log eerst in met eHerkenning om uw organisatiegegevens te bekijken."
        >
          <nldd-button
            slot="actions"
            appearance="primary"
            text="Naar inloggen"
            @click="router.push('/')"
          ></nldd-button>
        </nldd-inline-dialog>
      </nldd-simple-section>
    </template>

    <template v-else-if="session.aanvrager">
      <nldd-simple-section width="820px">
        <nldd-title size="display-2">
          <span slot="overline">{{ session.aanvrager.partij_naam }} · KVK {{ session.aanvrager.kvk_nummer }}</span>
          <h2>Mijn organisatie</h2>
        </nldd-title>
        <nldd-spacer size="24"></nldd-spacer>

        <!-- Registratie: de koppeling in het partijregister van de Napp -->
        <nldd-title size="display-3"><h3>Registratie</h3></nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-rich-text>
          <p>
            De koppeling tussen uw rechtspersoon en de geregistreerde
            aanduiding in het partijregister van de Napp. Uw zetels volgen
            uit de officiële verkiezingsuitslagen van de Kiesraad.
          </p>
        </nldd-rich-text>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-list v-if="registratie?.partij" appearance="box-tinted">
          <nldd-list-item size="sm">
            <nldd-text-cell text="Geregistreerde aanduiding" color="secondary"></nldd-text-cell>
            <nldd-text-cell :text="registratie.partij.naam" horizontal-alignment="right"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item size="sm">
            <nldd-text-cell text="Organisatiemodel" color="secondary"></nldd-text-cell>
            <nldd-text-cell
              :text="ORGANISATIEMODEL_LABELS[registratie.partij.organisatiemodel] ?? registratie.partij.organisatiemodel"
              horizontal-alignment="right"
            ></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item v-if="registratie.partij.kamerzetels" size="sm">
            <nldd-text-cell text="Kamerzetels (EK + TK) · bron: Kiesraad" color="secondary"></nldd-text-cell>
            <nldd-text-cell :text="String(registratie.partij.kamerzetels)" horizontal-alignment="right"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item size="sm">
            <nldd-text-cell text="Aanspraken volgens de uitslagen" color="secondary"></nldd-text-cell>
            <nldd-text-cell :text="String(registratie.aanspraken?.length ?? 0)" horizontal-alignment="right"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item v-if="beperkteMachtiging" size="sm">
            <nldd-text-cell text="Uw machtiging" color="secondary"></nldd-text-cell>
            <nldd-text-cell
              :text="`Beperkt: afdeling ${machtiging.gebied_naam}`"
              horizontal-alignment="right"
            ></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <NBanner
          v-else
          variant="warning"
          text="Geen registratie gevonden"
          supporting-text="Uw organisatie staat niet in het partijregister van de Napp. Heeft uw partij zetels behaald? Claim dan uw aanduiding via het aanvraagformulier."
        />

        <!-- Rekening voor uitbetaling: één per rechtspersoon (art. 27 Wpp) -->
        <nldd-spacer size="40"></nldd-spacer>
        <nldd-title size="display-3">
          <h3>Rekening voor uitbetaling</h3>
          <div
            v-if="rekening?.in_register && !beperkteMachtiging"
            slot="actions"
          >
            <nldd-button
              appearance="secondary"
              :text="rekening?.iban ? 'Rekening wijzigen' : 'Rekening opgeven'"
              start-icon="pencil"
              @click="openRekening"
            ></nldd-button>
          </div>
        </nldd-title>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-rich-text>
          <p>
            De subsidie wordt verstrekt aan de rechtspersoon (artikel 27 Wpp):
            er geldt één rekeningnummer per partij, op naam van de
            rechtspersoon.
          </p>
        </nldd-rich-text>
        <nldd-spacer size="12"></nldd-spacer>

        <template v-if="rekeningMelding">
          <NBanner variant="success" :text="rekeningMelding" />
          <nldd-spacer size="12"></nldd-spacer>
        </template>

        <nldd-list appearance="box-tinted">
          <nldd-list-item size="md">
            <nldd-title-cell
              :text="rekening?.iban ?? 'Nog niet opgegeven'"
              :supporting-text="
                rekening?.iban
                  ? `Tenaamstelling: ${rekening.tenaamstelling}`
                  : 'Voor uitbetaling van het voorschot is een rekeningnummer op naam van de rechtspersoon nodig.'
              "
            ></nldd-title-cell>
            <template v-if="rekening && !rekening.iban">
              <nldd-spacer-cell size="12"></nldd-spacer-cell>
              <nldd-cell width="fit-content">
                <nldd-tag color="warning" text="Ontbreekt"></nldd-tag>
              </nldd-cell>
            </template>
          </nldd-list-item>
        </nldd-list>

        <template v-if="beperkteMachtiging">
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-rich-text>
            <p>
              Alleen het tekenbevoegd bestuur van de partij kan het
              rekeningnummer opgeven of wijzigen. Met uw beperkte machtiging
              als afdelingsbestuurder kunt u het hier alleen inzien.
            </p>
          </nldd-rich-text>
        </template>
        <template v-else-if="rekening && !rekening.in_register">
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-rich-text>
            <p>
              Uw organisatie staat nog niet in het partijregister van de Napp.
              Zodra de registratie is vastgelegd kan het bestuur hier een
              rekeningnummer opgeven.
            </p>
          </nldd-rich-text>
        </template>
      </nldd-simple-section>

      <!-- Rekening opgeven of wijzigen -->
      <nldd-sheet
        ref="rekeningSheetEl"
        placement="right"
        width="480px"
        accessible-label="Rekening voor uitbetaling"
        @close="rekeningOpen = false"
      >
        <nldd-container padding="24" gap="16">
          <nldd-title size="display-3">
            <span slot="overline">{{ session.aanvrager?.partij_naam }}</span>
            <h3>Rekening voor uitbetaling</h3>
          </nldd-title>
          <nldd-rich-text>
            <p>
              Het IBAN wordt gevalideerd en de tenaamstelling wordt vergeleken
              met de geregistreerde aanduiding van uw partij
              (IBAN-naam-controle, in deze demo gesimuleerd).
            </p>
          </nldd-rich-text>
          <nldd-form novalidate @submit.prevent="bewaarRekening">
            <nldd-form-field label="IBAN">
              <nldd-text-field
                :value="rekeningForm.iban"
                name="iban"
                placeholder="NL00BANK0123456789"
                @input="rekeningForm.iban = veld($event)"
              ></nldd-text-field>
            </nldd-form-field>
            <nldd-form-field label="Tenaamstelling">
              <nldd-text-field
                :value="rekeningForm.tenaamstelling"
                name="tenaamstelling"
                :placeholder="session.aanvrager?.partij_naam"
                @input="rekeningForm.tenaamstelling = veld($event)"
              ></nldd-text-field>
              <nldd-form-field-help-text>
                De rekening moet op naam van de rechtspersoon staan, niet van
                een bestuurslid of afdeling.
              </nldd-form-field-help-text>
            </nldd-form-field>
            <template v-if="rekeningFout">
              <NBanner variant="critical" :text="rekeningFout" />
            </template>
            <nldd-form-actions>
              <nldd-button-group orientation="horizontal">
                <nldd-button
                  appearance="primary"
                  type="submit"
                  text="Opslaan"
                  :disabled="rekeningBezig || undefined"
                ></nldd-button>
                <nldd-button
                  appearance="secondary"
                  text="Annuleren"
                  @click="rekeningOpen = false"
                ></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </nldd-form>
        </nldd-container>
      </nldd-sheet>
    </template>
  </nldd-page>
</template>
