<script setup>
import { computed, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import PortalHeader from '../../components/PortalHeader.vue';
import NBanner from '../../components/NBanner.vue';
import ClaimAanduiding from '../../components/ClaimAanduiding.vue';
import { api } from '../../api.js';
import { session } from '../../session.js';
import { euro, datum, onderdelen } from '../../format.js';

const router = useRouter();

const registratie = ref(null);
const rekening = ref(null);
const geselecteerd = ref(new Set());
const leden = ref(0);

const heeftWetenschappelijkInstituut = ref(false);
const heeftJongerenorganisatie = ref(false);
const pjoLeden = ref(0);
const heeftInstellingBuitenland = ref(false);

const geenAnoniemeGiften = ref(false);
const geenGiftenNietIngezetenen = ref(false);
const meldplichtNageleefd = ref(false);
const financienOpenbaar = ref(false);

const fout = ref('');
const bezig = ref(false);
const proef = ref(null);
const proefFout = ref('');
let proefTimer = null;

// Inline rekening opgeven (alleen tekenbevoegd bestuur, zie rekening.rs).
const rekeningIban = ref('');
const rekeningTenaamstelling = ref('');
const rekeningFout = ref('');
const rekeningBezig = ref(false);
const rekeningMelding = ref('');

async function bewaarRekening() {
  rekeningFout.value = '';
  rekeningBezig.value = true;
  try {
    const resultaat = await api.mijnRekeningWijzigen({
      iban: rekeningIban.value.trim(),
      tenaamstelling: rekeningTenaamstelling.value.trim(),
    });
    rekening.value = await api.mijnRekening();
    const n = resultaat.geactiveerde_betaalopdrachten ?? 0;
    rekeningMelding.value =
      n > 0
        ? `Rekening ${rekening.value?.iban ?? ''} opgeslagen. ${n === 1 ? 'Eén aangehouden betaalopdracht is' : `${n} aangehouden betaalopdrachten zijn`} alsnog klaargezet voor uitbetaling.`
        : `Rekening ${rekening.value?.iban ?? ''} opgeslagen.`;
  } catch (e) {
    rekeningFout.value = e.message;
  } finally {
    rekeningBezig.value = false;
  }
}

const GROEPEN = [
  { soort: 'LANDELIJK', titel: 'Landelijk', kolom: 'Kamerzetels (EK + TK)' },
  { orgaan: 'GEMEENTERAAD', titel: 'Gemeenteraden', kolom: 'Raadszetels' },
  { orgaan: 'PROVINCIALE_STATEN', titel: 'Provinciale staten', kolom: 'Statenzetels' },
  { orgaan: 'EILANDSRAAD', titel: 'Eilandsraden (Caribisch Nederland)', kolom: 'Eilandsraadszetels' },
  { orgaan: 'WATERSCHAP', titel: 'Waterschappen', kolom: 'Zetels algemeen bestuur' },
];

const aanspraken = computed(() => registratie.value?.aanspraken ?? []);

// Branch login (beperkte machtiging): the server already filters the
// aanspraken; here we only adjust the texts around them.
const machtiging = computed(() => session.aanvrager?.machtiging ?? null);
const beperkt = computed(() => machtiging.value?.type === 'BEPERKT');
const overline = computed(() =>
  beperkt.value
    ? `${session.aanvrager?.partij_naam} · afdeling ${machtiging.value.gebied_naam}`
    : session.aanvrager?.partij_naam,
);

function groepLeden(groep) {
  return aanspraken.value.filter((a) =>
    groep.soort ? a.soort === groep.soort : a.orgaan === groep.orgaan,
  );
}

const zichtbareGroepen = computed(() => GROEPEN.filter((g) => groepLeden(g).length));

const aantalGeselecteerd = computed(() => geselecteerd.value.size);
const landelijkGeselecteerd = computed(() => geselecteerd.value.has('LANDELIJK'));

// Zolang de gebruiker nog niets heeft ingevuld, tonen we geen hard
// wetsoordeel ("Afwijzing") maar een neutrale uitnodiging.
const formulierAangeraakt = computed(
  () =>
    leden.value > 0 ||
    geenAnoniemeGiften.value ||
    geenGiftenNietIngezetenen.value ||
    meldplichtNageleefd.value ||
    financienOpenbaar.value,
);

const verklaringenCompleet = computed(
  () =>
    geenAnoniemeGiften.value &&
    geenGiftenNietIngezetenen.value &&
    meldplichtNageleefd.value &&
    financienOpenbaar.value,
);

function toggle(key) {
  const next = new Set(geselecteerd.value);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  geselecteerd.value = next;
}

function selecteerGroep(groep, aan) {
  const next = new Set(geselecteerd.value);
  for (const a of groepLeden(groep)) {
    if (a.status !== 'BESCHIKBAAR') continue;
    if (aan) {
      next.add(a.key);
    } else {
      next.delete(a.key);
    }
  }
  geselecteerd.value = next;
}

function statusLabel(a) {
  if (a.status === 'IN_BEHANDELING') return 'Loopt al';
  if (a.status === 'TOEGEKEND') return 'Toegekend';
  return null;
}

/** The number in a number field, or null while it is empty (design system 0.18). */
function num(event) {
  const v = event.detail?.value ?? event.target?.value;
  if (v === null || v === undefined || v === '') return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

function aanvraagParameters() {
  return {
    aantal_betalende_leden: leden.value,
    heeft_wetenschappelijk_instituut: heeftWetenschappelijkInstituut.value,
    heeft_jongerenorganisatie: heeftJongerenorganisatie.value,
    aantal_leden_jongerenorganisatie: pjoLeden.value,
    heeft_instelling_buitenland: heeftInstellingBuitenland.value,
    ontvangt_anonieme_giften: !geenAnoniemeGiften.value,
    ontvangt_giften_niet_ingezetenen: !geenGiftenNietIngezetenen.value,
    voldoet_aan_meldplicht_giften: meldplichtNageleefd.value,
    financien_openbaar_op_website: financienOpenbaar.value,
  };
}

async function verstuur() {
  fout.value = '';
  bezig.value = true;
  try {
    const result = await api.nieuweAanvraag({
      componenten: [...geselecteerd.value],
      parameters: aanvraagParameters(),
    });
    router.push(`/aanvraag/${result.id}`);
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

async function laadRegistratie() {
  if (!session.aanvrager) return;
  try {
    registratie.value = await api.mijnRegistratie();
    // Default: alles wat beschikbaar is staat aangevinkt.
    geselecteerd.value = new Set(
      (registratie.value?.aanspraken ?? [])
        .filter((a) => a.status === 'BESCHIKBAAR')
        .map((a) => a.key),
    );
  } catch {
    registratie.value = null;
  }
  // Informational only: applying without an account is allowed; the
  // betaalopdracht is then held until the board submits one.
  try {
    rekening.value = await api.mijnRekening();
  } catch {
    rekening.value = null;
  }
  if (!rekeningTenaamstelling.value) {
    rekeningTenaamstelling.value = session.aanvrager?.partij_naam ?? '';
  }
}

async function herberekenProef() {
  if (!geselecteerd.value.size) {
    proef.value = null;
    proefFout.value = '';
    return;
  }
  try {
    proef.value = await api.proefAanspraken({
      componenten: [...geselecteerd.value],
      parameters: aanvraagParameters(),
    });
    proefFout.value = '';
  } catch (e) {
    proef.value = null;
    proefFout.value = e.message;
  }
}

watch(
  [
    geselecteerd,
    leden,
    heeftWetenschappelijkInstituut,
    heeftJongerenorganisatie,
    pjoLeden,
    heeftInstellingBuitenland,
    geenAnoniemeGiften,
    geenGiftenNietIngezetenen,
    meldplichtNageleefd,
    financienOpenbaar,
  ],
  () => {
    clearTimeout(proefTimer);
    proefTimer = setTimeout(herberekenProef, 400);
  },
);

onMounted(laadRegistratie);
watch(() => session.aanvrager, laadRegistratie);
</script>

<template>
  <nldd-page>
    <PortalHeader
      slot="header"
      portal="aanvrager"
      :items="[{ text: 'Mijn aanvragen', to: '/' }, { text: 'Nieuwe aanvraag', to: '/nieuw' }, { text: 'Mijn organisatie', to: '/organisatie' }]"
    />

    <template v-if="session.loaded && !session.aanvrager">
      <nldd-simple-section width="560px">
        <nldd-inline-dialog
          variant="alert"
          text="U bent niet ingelogd"
          supporting-text="Log eerst in met eHerkenning om een aanvraag in te dienen."
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

    <template v-else>
      <nldd-simple-section width="820px">
        <nldd-title size="display-2">
          <span slot="overline">{{ overline }}</span>
          <h2>Subsidie aanvragen voor {{ registratie?.subsidiejaar ?? '…' }}</h2>
        </nldd-title>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-rich-text>
          <p v-if="beperkt">
            U bent ingelogd als afdelingsbestuurder met een beperkte machtiging.
            U ziet alleen de aanspraken van uw afdeling en vraagt die namens de
            partij aan; de Napp beslist in één beschikking.
          </p>
          <p v-else>
            Dit zijn uw aanspraken volgens het partijregister, gebaseerd op de
            verkiezingsuitslagen van de Kiesraad. U vraagt alles in één keer aan;
            onderdelen uitvinken kan. De Napp beslist in één beschikking met een
            specificatie per onderdeel.
          </p>
          <p v-if="registratie?.aanvraagtermijn_einddatum">
            De subsidie geldt per kalenderjaar. Aanvragen voor
            {{ registratie.subsidiejaar }} kan tot en met
            {{ datum(registratie.aanvraagtermijn_einddatum) }}; de Napp besluit
            vóór 1 januari {{ registratie.subsidiejaar }} (artikel 17 Wpp). Bij
            verlening ontvangt u van rechtswege een voorschot van 80%.
          </p>
        </nldd-rich-text>
        <nldd-spacer size="24"></nldd-spacer>

        <template v-if="beperkt">
          <NBanner
            variant="neutral"
            text="Beperkte machtiging"
            :supporting-text="`Uw volmacht geldt voor ${machtiging.gebied_naam}. De transparantieverklaringen legt u met die volmacht namens de hele partij af; het ledental is alleen relevant voor de landelijke subsidie en blijft hier buiten beschouwing.`"
          />
          <nldd-spacer size="16"></nldd-spacer>
        </template>

        <!-- Niet in het register: claim-flow (vervangt de kale banner).
             Het claim-blok toont de zoeklijst met ongekoppelde aanduidingen,
             of de status van de eigen claim. -->
        <template v-if="registratie && !registratie.partij">
          <ClaimAanduiding />
          <nldd-spacer size="24"></nldd-spacer>
        </template>

        <!-- Rekening inline opgeven: alleen voor het tekenbevoegd bestuur
             van een geregistreerde rechtspersoon; anders een informatieve
             banner. Aanvragen kan ook zonder rekening (de betaalopdracht
             wordt dan aangehouden). -->
        <template v-if="rekeningMelding">
          <NBanner variant="success" :text="rekeningMelding" />
          <nldd-spacer size="16"></nldd-spacer>
        </template>
        <template v-if="rekening && !rekening.iban">
          <nldd-card
            v-if="!beperkt && rekening.in_register !== false"
            accessible-label="Rekening voor uitbetaling opgeven"
          >
            <nldd-container padding="20" gap="8">
              <nldd-title size="display-5"><h4>Nog geen rekeningnummer bekend</h4></nldd-title>
              <nldd-rich-text>
                <p>
                  Voor uitbetaling van het voorschot is een rekeningnummer op
                  naam van de rechtspersoon nodig. U kunt het hier direct
                  opgeven; aanvragen kan ook alvast zonder.
                </p>
              </nldd-rich-text>
              <nldd-form novalidate @submit.prevent="bewaarRekening">
                <nldd-form-field label="IBAN">
                  <nldd-text-field
                    :value="rekeningIban"
                    name="iban"
                    placeholder="NL00BANK0123456789"
                    @input="rekeningIban = $event.detail?.value ?? $event.target?.value ?? ''"
                  ></nldd-text-field>
                </nldd-form-field>
                <nldd-form-field label="Tenaamstelling">
                  <nldd-text-field
                    :value="rekeningTenaamstelling"
                    name="tenaamstelling"
                    @input="rekeningTenaamstelling = $event.detail?.value ?? $event.target?.value ?? ''"
                  ></nldd-text-field>
                  <nldd-form-field-help-text>
                    De rekening moet op naam van de rechtspersoon staan; de
                    Napp voert een IBAN-naam-controle uit.
                  </nldd-form-field-help-text>
                </nldd-form-field>
                <template v-if="rekeningFout">
                  <NBanner variant="critical" :text="rekeningFout" />
                  <nldd-spacer size="8"></nldd-spacer>
                </template>
                <nldd-form-actions>
                  <nldd-button
                    appearance="secondary"
                    type="submit"
                    text="Rekening opslaan"
                    :disabled="rekeningBezig || undefined"
                  ></nldd-button>
                </nldd-form-actions>
              </nldd-form>
            </nldd-container>
          </nldd-card>
          <NBanner
            v-else
            variant="neutral"
            text="Nog geen rekeningnummer bekend"
            :supporting-text="beperkt
              ? 'Voor uitbetaling van het voorschot is een rekeningnummer op naam van de rechtspersoon nodig; alleen het tekenbevoegd bestuur kan dat opgeven. U kunt wel alvast aanvragen.'
              : 'Voor uitbetaling van het voorschot is een rekeningnummer op naam van de rechtspersoon nodig; dat kan zodra uw organisatie aan een aanduiding is gekoppeld. U kunt wel alvast aanvragen.'"
          />
          <nldd-spacer size="16"></nldd-spacer>
        </template>

        <template v-for="groep in zichtbareGroepen" :key="groep.titel">
          <nldd-title size="display-4">
            <h3>{{ groep.titel }}</h3>
            <div v-if="groepLeden(groep).length > 1" slot="actions">
              <nldd-button-group orientation="horizontal">
                <nldd-button
                  size="sm"
                  appearance="neutral-transparent"
                  text="Alles"
                  @click="selecteerGroep(groep, true)"
                ></nldd-button>
                <nldd-button
                  size="sm"
                  appearance="neutral-transparent"
                  text="Niets"
                  @click="selecteerGroep(groep, false)"
                ></nldd-button>
              </nldd-button-group>
            </div>
          </nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-list appearance="box-tinted">
            <nldd-list-item v-for="a in groepLeden(groep)" :key="a.key" size="sm">
              <nldd-cell width="fit-content">
                <nldd-checkbox
                  :checked="geselecteerd.has(a.key) || undefined"
                  :disabled="a.status !== 'BESCHIKBAAR' || undefined"
                  :accessible-label="`Onderdeel ${a.gebied ?? 'landelijk'}`"
                  @change="toggle(a.key)"
                ></nldd-checkbox>
              </nldd-cell>
              <nldd-spacer-cell size="12"></nldd-spacer-cell>
              <nldd-text-cell
                :text="a.soort === 'LANDELIJK' ? 'Landelijke subsidie' : a.gebied"
                :supporting-text="`${groep.kolom}: ${a.zetels} · bron: Kiesraad`"
              ></nldd-text-cell>
              <template v-if="statusLabel(a)">
                <nldd-cell width="fit-content">
                  <nldd-tag
                    :color="a.status === 'TOEGEKEND' ? 'success' : 'warning'"
                    :text="statusLabel(a)"
                  ></nldd-tag>
                </nldd-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
              </template>
            </nldd-list-item>
          </nldd-list>
          <nldd-spacer size="24"></nldd-spacer>
        </template>

        <template v-if="landelijkGeselecteerd">
          <nldd-title size="display-4"><h3>Ledental</h3></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-form-field label="Betalende leden">
            <nldd-number-field
              :value="leden"
              name="leden"
              min="0"
              step="100"
              @input="leden = num($event) ?? leden"
              @change="leden = num($event) ?? leden"
            ></nldd-number-field>
            <nldd-form-field-help-text>
              Eigen opgave, alleen vereist voor de landelijke subsidie: leden
              met vergader- en stemrecht die jaarlijks ten minste € 12
              contributie betalen (minimaal 1.000). Voor decentrale
              onderdelen geldt geen ledeneis. Het ledenbudget wordt naar rato
              van de opgegeven ledentallen verdeeld over alle ontvangende
              partijen (artikel 14).
            </nldd-form-field-help-text>
          </nldd-form-field>
          <nldd-spacer size="24"></nldd-spacer>

          <nldd-title size="display-4"><h3>Neveninstellingen</h3></nldd-title>
          <nldd-spacer size="4"></nldd-spacer>
          <nldd-rich-text>
            <p>
              Voor aangewezen neveninstellingen ontvangt de partij een
              geoormerkt bedrag dat zij doorbetaalt (artikel 14, onderdelen
              b tot en met d).
            </p>
          </nldd-rich-text>
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-container gap="12">
            <nldd-checkbox-field
              label="Onze partij heeft een politiek-wetenschappelijk instituut aangewezen (artikel 3)"
              :checked="heeftWetenschappelijkInstituut || undefined"
              @change="heeftWetenschappelijkInstituut = $event.detail?.checked ?? false"
            ></nldd-checkbox-field>
            <nldd-checkbox-field
              label="Onze partij heeft een politieke jongerenorganisatie aangewezen (artikel 4)"
              :checked="heeftJongerenorganisatie || undefined"
              @change="heeftJongerenorganisatie = $event.detail?.checked ?? false"
            ></nldd-checkbox-field>
            <template v-if="heeftJongerenorganisatie">
              <nldd-form-field label="Leden van de jongerenorganisatie">
                <nldd-number-field
                  :value="pjoLeden"
                  name="pjo_leden"
                  min="0"
                  step="100"
                  @input="pjoLeden = num($event) ?? pjoLeden"
                  @change="pjoLeden = num($event) ?? pjoLeden"
                ></nldd-number-field>
                <nldd-form-field-help-text>
                  Eigen opgave; voor de jongerenorganisatie geldt een vast
                  bedrag per lid.
                </nldd-form-field-help-text>
              </nldd-form-field>
            </template>
            <nldd-checkbox-field
              label="Onze partij heeft een instelling voor buitenlandse activiteiten aangewezen"
              :checked="heeftInstellingBuitenland || undefined"
              @change="heeftInstellingBuitenland = $event.detail?.checked ?? false"
            ></nldd-checkbox-field>
          </nldd-container>
          <nldd-spacer size="24"></nldd-spacer>
        </template>

        <nldd-title size="display-4"><h3>Transparantieverklaringen</h3></nldd-title>
        <nldd-spacer size="4"></nldd-spacer>
        <nldd-rich-text>
          <p>De verklaringen gelden voor de hele rechtspersoon (artikel 5 Wpp).</p>
        </nldd-rich-text>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-container gap="12">
          <nldd-checkbox-field
            label="Onze partij ontvangt geen anonieme giften"
            :checked="geenAnoniemeGiften || undefined"
            @change="geenAnoniemeGiften = $event.detail?.checked ?? false"
          ></nldd-checkbox-field>
          <nldd-checkbox-field
            label="Onze partij ontvangt geen giften van niet-ingezetenen"
            :checked="geenGiftenNietIngezetenen || undefined"
            @change="geenGiftenNietIngezetenen = $event.detail?.checked ?? false"
          ></nldd-checkbox-field>
          <nldd-checkbox-field
            label="Giften van € 10.000 of meer melden wij binnen de termijn"
            :checked="meldplichtNageleefd || undefined"
            @change="meldplichtNageleefd = $event.detail?.checked ?? false"
          ></nldd-checkbox-field>
          <nldd-checkbox-field
            label="Onze financiën staan openbaar op onze website"
            :checked="financienOpenbaar || undefined"
            @change="financienOpenbaar = $event.detail?.checked ?? false"
          ></nldd-checkbox-field>
        </nldd-container>
        <nldd-spacer size="24"></nldd-spacer>

        <template v-if="proefFout">
          <NBanner
            variant="critical"
            text="Indicatieve berekening niet mogelijk"
            :supporting-text="proefFout"
          />
          <nldd-spacer size="16"></nldd-spacer>
        </template>
        <template v-else-if="proef && !formulierAangeraakt">
          <nldd-box>
            <nldd-container padding="16" gap="8">
              <nldd-title size="display-4">
                <span slot="overline">Indicatieve uitkomst volgens de wet</span>
                <h4>Vul het formulier in</h4>
              </nldd-title>
              <nldd-rich-text>
                <p>
                  Deze indicatie rekent live mee met wat u invult: zodra u uw
                  ledental en verklaringen opgeeft, ziet u hier wat de wet
                  van uw aanvraag vindt, nog vóór u indient.
                </p>
              </nldd-rich-text>
            </nldd-container>
          </nldd-box>
          <nldd-spacer size="16"></nldd-spacer>
        </template>
        <template v-else-if="proef">
          <nldd-box>
            <nldd-container padding="16" gap="8">
              <nldd-title size="display-4">
                <span slot="overline">Indicatieve uitkomst volgens de wet</span>
                <h4>{{ proef.subsidie_toegekend ? euro(proef.subsidiebedrag) : 'Nog geen toekenning' }}</h4>
              </nldd-title>
              <nldd-rich-text>
                <p>
                  {{ proef.subsidie_toegekend
                    ? `${proef.onderdelen_toegekend} van ${onderdelen(proef.onderdelen_totaal)} toegekend · voorschot 80%: ${euro(proef.voorschot_bedrag)} · dit is geen besluit`
                    : `${proef.onderdelen_toegekend} van ${onderdelen(proef.onderdelen_totaal)} toegekend · dit is geen besluit` }}
                </p>
              </nldd-rich-text>
              <!-- Bij afgewezen onderdelen legt de motivering uit welke en
                   waarom -- dezelfde tekst die straks in het besluit staat. -->
              <template v-if="proef.motivering && proef.onderdelen_toegekend < proef.onderdelen_totaal">
                <nldd-divider></nldd-divider>
                <nldd-rich-text>
                  <p>{{ proef.motivering }}</p>
                </nldd-rich-text>
              </template>
            </nldd-container>
          </nldd-box>
          <nldd-spacer size="16"></nldd-spacer>
        </template>

        <nldd-button-group orientation="horizontal">
          <nldd-button
            appearance="primary"
            :text="`Aanvraag indienen (${onderdelen(aantalGeselecteerd)})`"
            start-icon="paper-plane"
            :disabled="bezig || aantalGeselecteerd === 0 || undefined"
            @click="verstuur"
          ></nldd-button>
          <nldd-button
            appearance="neutral-transparent"
            text="Annuleren"
            @click="router.push('/')"
          ></nldd-button>
        </nldd-button-group>

        <template v-if="fout">
          <nldd-spacer size="16"></nldd-spacer>
          <NBanner variant="critical" text="Indienen mislukt" :supporting-text="fout" />
        </template>
      </nldd-simple-section>
    </template>
  </nldd-page>
</template>
