<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import PortalHeader from '../../components/PortalHeader.vue';
import NBanner from '../../components/NBanner.vue';
import { api } from '../../api.js';
import { session, refreshSession } from '../../session.js';
import { euro, datum, onderdelen, statusLabel, statusColor } from '../../format.js';

const router = useRouter();

const kvk = ref('');
const loginFout = ref('');
const bezig = ref(false);

// Two-step login: after the KvK number the mocked eHerkenning may offer
// multiple representation profiles (full board vs. branch volmacht).
const stap = ref('kvk'); // 'kvk' | 'machtiging'
const partijNaam = ref('');
const profielen = ref([]);
const afdelingKiezen = ref(false);
const zoek = ref('');

const aanvragen = ref([]);
const laden = ref(false);
const demoVoorbeelden = ref([]);

const ORGAAN_LABEL = {
  GEMEENTERAAD: 'Gemeenteraad',
  PROVINCIALE_STATEN: 'Provinciale staten',
  WATERSCHAP: 'Waterschap',
};

function kiesDemo(d) {
  kvk.value = d.kvk_nummer;
  login();
}

const navItems = computed(() =>
  session.aanvrager
    ? [
        { text: 'Mijn aanvragen', to: '/' },
        { text: 'Nieuwe aanvraag', to: '/nieuw' },
        { text: 'Mijn organisatie', to: '/organisatie' },
      ]
    : [],
);

const beperkteProfielen = computed(() =>
  profielen.value.filter((p) => p.type === 'BEPERKT'),
);

const aanvragerOverline = computed(() => {
  const a = session.aanvrager;
  if (!a) return '';
  const afdeling =
    a.machtiging?.type === 'BEPERKT' ? ` · afdeling ${a.machtiging.gebied_naam}` : '';
  return `${a.partij_naam}${afdeling} · KVK ${a.kvk_nummer}`;
});

const MAX_GETOOND = 12;
const gefilterdeProfielen = computed(() => {
  const q = zoek.value.trim().toLowerCase();
  return q
    ? beperkteProfielen.value.filter((p) =>
        p.gebied_naam.toLowerCase().includes(q),
      )
    : beperkteProfielen.value;
});
const getoondeProfielen = computed(() =>
  gefilterdeProfielen.value.slice(0, MAX_GETOOND),
);
const restAantal = computed(
  () => gefilterdeProfielen.value.length - getoondeProfielen.value.length,
);

async function login() {
  loginFout.value = '';
  if (!/^\d{8}$/.test(kvk.value.trim())) {
    loginFout.value = 'Vul een geldig KVK-nummer in (8 cijfers).';
    return;
  }
  bezig.value = true;
  try {
    const result = await api.eherkenningMachtigingen(kvk.value.trim());
    profielen.value = result.profielen ?? [];
    partijNaam.value = result.partij_naam ?? `Organisatie ${kvk.value.trim()}`;
    if (beperkteProfielen.value.length) {
      // The party knows branch volmachten: ask on whose behalf to log in.
      stap.value = 'machtiging';
      afdelingKiezen.value = false;
      zoek.value = '';
    } else {
      await doLogin(null);
    }
  } catch (e) {
    loginFout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

async function doLogin(machtiging) {
  loginFout.value = '';
  bezig.value = true;
  try {
    await api.eherkenningLogin(kvk.value.trim(), machtiging);
    await refreshSession();
    stap.value = 'kvk';
    profielen.value = [];
  } catch (e) {
    loginFout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

function terugNaarKvk() {
  stap.value = 'kvk';
  profielen.value = [];
  afdelingKiezen.value = false;
  zoek.value = '';
  loginFout.value = '';
}

async function laadAanvragen() {
  if (!session.aanvrager) return;
  laden.value = true;
  try {
    aanvragen.value = await api.mijnAanvragen();
  } finally {
    laden.value = false;
  }
}

onMounted(async () => {
  laadAanvragen();
  try {
    demoVoorbeelden.value = await api.registerDemo();
  } catch {
    demoVoorbeelden.value = [];
  }
});
watch(() => session.aanvrager, laadAanvragen);
</script>

<template>
  <nldd-page>
    <PortalHeader
      slot="header"
      :items="navItems"
      portal="aanvrager"
    />

    <!-- Niet ingelogd: de gesimuleerde eHerkenning-omgeving. In het echt
         verlaat de gebruiker het portaal (omgeving van de middelenleverancier
         en het machtigingenregister) en keert daarna terug; dat externe
         karakter is hier visueel nagebootst. -->
    <template v-if="session.loaded && !session.aanvrager">
      <nldd-simple-section width="560px">
        <template v-if="stap === 'kvk'">
          <nldd-title size="2">
            <span slot="overline">Voor politieke partijen</span>
            <h2>Inloggen met eHerkenning</h2>
          </nldd-title>
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-rich-text>
            <p>
              Politieke partijen zijn verenigingen of stichtingen en loggen in
              met eHerkenning namens hun organisatie. U verlaat tijdelijk het
              subsidieportaal.
            </p>
          </nldd-rich-text>
          <nldd-spacer size="24"></nldd-spacer>
        </template>

        <div class="eh-omgeving">
          <div class="eh-kop">
            <span class="eh-logo">eHerkenning</span>
            <span class="eh-tag">gesimuleerde omgeving</span>
          </div>
          <div class="eh-dienst">
            U logt in bij: <strong>Nederlandse autoriteit politieke partijen</strong><br />
            Dienst: Subsidiezaken politieke partijen · betrouwbaarheidsniveau EH3
          </div>

          <div class="eh-body">
            <!-- Stap 1: identificatie van de rechtspersoon (mock: KvK-nummer
                 in plaats van een persoonlijk middel bij een leverancier). -->
            <template v-if="stap === 'kvk'">
              <nldd-form novalidate @submit.prevent="login">
                <nldd-form-field label="KVK-nummer van uw organisatie">
                  <nldd-text-field
                    name="kvk"
                    :value="kvk"
                    placeholder="12345678"
                    :invalid="loginFout.includes('KVK') || undefined"
                    unmet="kvk-fout"
                    @input="kvk = $event.detail?.value ?? $event.target?.value ?? ''"
                  ></nldd-text-field>
                  <nldd-validation-list>
                    <nldd-validation-item id="kvk-fout">
                      Vul een geldig KVK-nummer in (8 cijfers).
                    </nldd-validation-item>
                  </nldd-validation-list>
                  <nldd-form-field-help-text>
                    Gesimuleerd: in het echt logt u hier in met uw
                    persoonlijke eHerkenning-middel; nu telt alleen het
                    KVK-nummer. Elk ander nummer logt in als ongeregistreerde
                    organisatie.
                  </nldd-form-field-help-text>
                </nldd-form-field>
                <nldd-form-actions>
                  <nldd-button
                    appearance="primary"
                    type="submit"
                    text="Verder"
                    start-icon="lock-closed"
                    :disabled="bezig || undefined"
                  ></nldd-button>
                </nldd-form-actions>
              </nldd-form>
            </template>

            <!-- Stap 2: machtigingskeuze. In het echt staat dit in het
                 machtigingenregister (toegekend door het tekenbevoegd
                 bestuur); hier kiest u er vrij een voor de demo. -->
            <template v-else>
              <nldd-title size="4">
                <span slot="overline">{{ partijNaam }} · KVK {{ kvk }}</span>
                <h3>Uw machtigingen voor deze dienst</h3>
              </nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-rich-text>
                <p>
                  Deze partij is centraal georganiseerd: afdelingen vallen
                  onder de landelijke rechtspersoon. In het echt bepaalt het
                  machtigingenregister welke machtiging u heeft (toegekend
                  door het tekenbevoegd bestuur, volmacht art. 27 Wpp); in
                  deze demo kiest u er zelf een.
                </p>
              </nldd-rich-text>
              <nldd-spacer size="16"></nldd-spacer>

              <nldd-list appearance="box-tinted">
                <nldd-list-item size="md" button @click="doLogin({ type: 'VOLLEDIG' })">
                  <nldd-title-cell
                    text="De gehele partij"
                    supporting-text="Tekenbevoegd bestuur: alle aanspraken, landelijk en decentraal"
                  ></nldd-title-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-icon-cell icon="chevron-right" size="16"></nldd-icon-cell>
                </nldd-list-item>
                <nldd-list-item size="md" button @click="afdelingKiezen = !afdelingKiezen">
                  <nldd-title-cell
                    text="Een afdeling"
                    :supporting-text="`Beperkte machtiging voor één gebied (${beperkteProfielen.length} gebieden)`"
                  ></nldd-title-cell>
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-icon-cell
                    :icon="afdelingKiezen ? 'chevron-up' : 'chevron-down'"
                    size="16"
                  ></nldd-icon-cell>
                </nldd-list-item>
              </nldd-list>

              <template v-if="afdelingKiezen">
                <nldd-spacer size="16"></nldd-spacer>
                <nldd-form-field label="Zoek uw gebied">
                  <nldd-text-field
                    name="zoek-gebied"
                    :value="zoek"
                    placeholder="Bijvoorbeeld: Utrecht"
                    @input="zoek = $event.detail?.value ?? $event.target?.value ?? ''"
                  ></nldd-text-field>
                </nldd-form-field>
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-list v-if="getoondeProfielen.length" appearance="box-tinted">
                  <nldd-list-item
                    v-for="p in getoondeProfielen"
                    :key="`${p.orgaan}:${p.gebied_code}`"
                    size="sm"
                    button
                    @click="doLogin({ type: 'BEPERKT', gebied_code: p.gebied_code })"
                  >
                    <nldd-text-cell
                      :text="p.gebied_naam"
                      :supporting-text="ORGAAN_LABEL[p.orgaan] ?? p.orgaan"
                    ></nldd-text-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <nldd-icon-cell icon="chevron-right" size="16"></nldd-icon-cell>
                  </nldd-list-item>
                </nldd-list>
                <nldd-rich-text v-if="restAantal > 0">
                  <p>Nog {{ restAantal }} gebieden. Verfijn uw zoekopdracht.</p>
                </nldd-rich-text>
                <nldd-rich-text v-if="!getoondeProfielen.length">
                  <p>Geen gebied gevonden voor &lsquo;{{ zoek }}&rsquo;.</p>
                </nldd-rich-text>
              </template>

              <nldd-spacer size="16"></nldd-spacer>
              <nldd-button
                appearance="neutral-transparent"
                text="Terug"
                start-icon="arrow-left"
                :disabled="bezig || undefined"
                @click="terugNaarKvk"
              ></nldd-button>
            </template>

            <template v-if="loginFout">
              <nldd-spacer size="16"></nldd-spacer>
              <NBanner variant="critical" :text="loginFout" />
            </template>
          </div>

          <div class="eh-voet">
            Na het inloggen keert u terug naar het subsidieportaal van de Napp.
          </div>
        </div>

        <template v-if="stap === 'kvk' && demoVoorbeelden.length">
          <nldd-spacer size="32"></nldd-spacer>
          <nldd-title size="4"><h3>Voorbeeldpartijen (demo)</h3></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-list appearance="box-tinted">
            <nldd-list-item
              v-for="d in demoVoorbeelden"
              :key="d.kvk_nummer"
              size="sm"
              button
              @click="kiesDemo(d)"
            >
              <nldd-text-cell
                :text="d.naam"
                :supporting-text="d.profiel"
              ></nldd-text-cell>
              <nldd-text-cell
                :text="d.kvk_nummer"
                color="secondary"
                horizontal-alignment="right"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-icon-cell icon="chevron-right" size="16"></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>
        </template>
      </nldd-simple-section>
    </template>

    <!-- Ingelogd: eigen aanvragen -->
    <template v-else-if="session.aanvrager">
      <nldd-simple-section>
        <nldd-title size="2">
          <span slot="overline">{{ aanvragerOverline }}</span>
          <h2>Uw subsidieaanvragen</h2>
          <div v-if="aanvragen.length" slot="actions">
            <nldd-button
              appearance="primary"
              text="Nieuwe aanvraag"
              start-icon="plus"
              @click="router.push('/nieuw')"
            ></nldd-button>
          </div>
        </nldd-title>
        <nldd-spacer size="24"></nldd-spacer>

        <nldd-list v-if="aanvragen.length" appearance="box-tinted">
          <nldd-list-item
            v-for="item in aanvragen"
            :key="item.aanvraag.id"
            size="md"
            button
            @click="router.push(`/aanvraag/${item.aanvraag.id}`)"
          >
            <nldd-title-cell
              :text="`Jaaraanvraag ${item.aanvraag.subsidiejaar}`"
              :supporting-text="`${onderdelen(item.aanvraag.componenten.length)} · ingediend op ${datum(item.aanvraag.aanvraag_datum)}`"
            ></nldd-title-cell>
            <nldd-text-cell
              v-if="item.besluit"
              width="fit-content"
              :text="euro(item.besluit.subsidiebedrag)"
              horizontal-alignment="right"
            ></nldd-text-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-cell width="fit-content">
              <nldd-tag
                :color="statusColor(item.aanvraag.status, item.besluit)"
                :text="statusLabel(item.aanvraag.status, item.besluit)"
              ></nldd-tag>
            </nldd-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
          </nldd-list-item>
        </nldd-list>

        <nldd-inline-dialog
          v-else-if="!laden"
          icon="inbox"
          text="Nog geen aanvragen"
          supporting-text="Dien uw eerste subsidieaanvraag in. U ziet direct of uw partij aan de voorwaarden voldoet."
        >
          <nldd-button
            slot="actions"
            appearance="primary"
            text="Nieuwe aanvraag"
            @click="router.push('/nieuw')"
          ></nldd-button>
        </nldd-inline-dialog>

      </nldd-simple-section>
    </template>
  </nldd-page>
</template>

<style scoped>
/* Gesimuleerde eHerkenning-omgeving: bewust een vreemde eend in het portaal
   (eigen merk-kop in eHerkenning-magenta, #DE0367 uit hun huisstijl), zodat
   zichtbaar is dat de gebruiker het portaal tijdelijk verlaat. Vaste
   merkkleuren: dit bootst een extern merk na en hoort niet mee te kleuren
   met het portaalthema. Tekst-styling volgt de design-system-tokens zodat
   de kale elementen niet terugvallen op het browser-serif.
   eHerkenning gebruikt zelf een eigen sans-serif (Gilroy); die benaderen we
   met de systeem-sans in plaats van een font te embedden. */
.eh-omgeving {
  font: var(--primitives-font-body-md-regular-snug);
  font-family: helvetica, roboto, arial, sans-serif;
  border: 1px solid light-dark(var(--primitives-color-neutral-300), var(--primitives-color-neutral-700));
  border-radius: 8px;
  overflow: hidden;
  background: light-dark(#ffffff, var(--primitives-color-neutral-900));
}
.eh-kop {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: var(--primitives-space-8);
  padding: var(--primitives-space-12) var(--primitives-space-16);
  background: #de0367;
  color: #ffffff;
}
.eh-logo {
  font-weight: 700;
  font-size: 1.05rem;
  letter-spacing: 0.01em;
}
.eh-tag {
  font-size: 0.8rem;
  opacity: 0.9;
}
.eh-dienst {
  padding: var(--primitives-space-12) var(--primitives-space-16);
  font-size: 0.875rem;
  line-height: 1.5;
  background: light-dark(var(--primitives-color-neutral-100), var(--primitives-color-neutral-800));
  border-bottom: 1px solid light-dark(var(--primitives-color-neutral-300), var(--primitives-color-neutral-700));
}
.eh-body {
  padding: var(--primitives-space-20) var(--primitives-space-16);
}
.eh-voet {
  padding: var(--primitives-space-12) var(--primitives-space-16);
  font-size: 0.8125rem;
  color: light-dark(var(--primitives-color-neutral-600), var(--primitives-color-neutral-400));
  border-top: 1px solid light-dark(var(--primitives-color-neutral-300), var(--primitives-color-neutral-700));
}
</style>
