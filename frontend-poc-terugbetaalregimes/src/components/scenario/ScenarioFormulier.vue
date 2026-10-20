<template>
  <nldd-sheet
    ref="sheetEl"
    placement="right"
    width="640px"
    accessible-label="Scenario toevoegen"
    @close="emit('close')"
  >
    <nldd-page>
      <nldd-container padding="24" gap="24">
        <nldd-title :size="3">
          <span slot="overline">Voor beleidsmakers</span>
          <span>{{ bewerken ? 'Scenario aanpassen' : 'Scenario toevoegen' }}</span>
          <span slot="supporting-text">
            Stel een situatie samen en reken hem door naast de andere scenario's.
          </span>
        </nldd-title>

        <div class="sf-velden">
          <nldd-form-field label="Naam" supporting-label="zo heet dit scenario in de tabel">
            <nldd-text-field
              :value="vorm.naam"
              size="sm"
              @input="vorm.naam = $event.detail?.value ?? vorm.naam"
            ></nldd-text-field>
          </nldd-form-field>

          <div class="sf-rij">
            <nldd-form-field label="Studieschuld (€)" supporting-label="bij de start van het terugbetalen">
              <nldd-number-field
                :value="vorm.schuld"
                :min="0"
                :step="1000"
                size="sm"
                @input="vorm.schuld = num($event, vorm.schuld)"
              ></nldd-number-field>
            </nldd-form-field>
            <nldd-form-field label="Inkomen per jaar (€)" supporting-label="bruto">
              <nldd-number-field
                :value="vorm.inkomen"
                :min="0"
                :step="1000"
                size="sm"
                @input="vorm.inkomen = num($event, vorm.inkomen)"
              ></nldd-number-field>
            </nldd-form-field>
          </div>

          <nldd-form-field label="Huishouden">
            <nldd-dropdown
              :key="`hh:${vorm.huishoudtype}`"
              size="sm"
              width="100%"
              @change="vorm.huishoudtype = $event.detail?.value ?? vorm.huishoudtype"
            >
              <select :value="vorm.huishoudtype" aria-label="Huishouden">
                <option v-for="(label, key) in HUISHOUD" :key="key" :value="key">{{ label }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>

          <nldd-form-field
            v-if="heeftPartner"
            label="Inkomen partner per jaar (€)"
            supporting-label="telt mee bij het bepalen van je maandbedrag"
          >
            <nldd-number-field
              :value="vorm.partnerinkomen"
              :min="0"
              :step="1000"
              size="sm"
              @input="vorm.partnerinkomen = num($event, vorm.partnerinkomen)"
            ></nldd-number-field>
          </nldd-form-field>

          <div class="sf-rij">
            <nldd-form-field label="Begon met studiefinanciering in" supporting-label="jaartal">
              <nldd-number-field
                :value="vorm.startjaar"
                :min="1986"
                :max="2030"
                size="sm"
                @input="vorm.startjaar = num($event, vorm.startjaar)"
              ></nldd-number-field>
            </nldd-form-field>
            <nldd-form-field label="Opleiding">
              <nldd-dropdown
                :key="`op:${vorm.onderwijssoort}`"
                size="sm"
                width="100%"
                @change="vorm.onderwijssoort = $event.detail?.value ?? vorm.onderwijssoort"
              >
                <select :value="vorm.onderwijssoort" aria-label="Opleiding">
                  <option v-for="(label, key) in ONDERWIJS" :key="key" :value="key">{{ label }}</option>
                </select>
              </nldd-dropdown>
            </nldd-form-field>
          </div>

          <nldd-switch-field
            label="Dit is een levenlanglerenkrediet"
            :checked="vorm.lllk ? true : undefined"
            @change="vorm.lllk = $event.detail?.checked ?? vorm.lllk"
          ></nldd-switch-field>

          <!-- Het regime kies je niet; het volgt uit je cohort. Dat laten zien
               is precies het punt dat deze casus wil maken. -->
          <div class="sf-regime">
            <span class="sf-regime-label">Hiermee gelden de regels van</span>
            <nldd-tag :color="regimeColor(regime)" size="sm" :text="regimeLabel(regime)"></nldd-tag>
            <span class="sf-regime-uitleg">{{ cohortUitleg(alsRecord) }}</span>
          </div>

          <details class="sf-meer">
            <summary>Meer instellen</summary>
            <div class="sf-velden sf-meer-velden">
              <div class="sf-rij">
                <nldd-form-field label="Geboortejaar" supporting-label="voor de leeftijdsgrens in de berekening">
                  <nldd-number-field
                    :value="vorm.geboortejaar"
                    :min="1940"
                    :max="2012"
                    size="sm"
                    @input="vorm.geboortejaar = num($event, vorm.geboortejaar)"
                  ></nldd-number-field>
                </nldd-form-field>
                <nldd-form-field label="Inkomensgroei per jaar (%)" supporting-label="over de hele looptijd">
                  <nldd-number-field
                    :value="vorm.groei"
                    :min="-5"
                    :max="10"
                    :step="0.5"
                    size="sm"
                    @input="vorm.groei = num($event, vorm.groei)"
                  ></nldd-number-field>
                </nldd-form-field>
              </div>
            </div>
          </details>
        </div>

        <!-- Meteen zien of je iets zinnigs hebt ingevuld. -->
        <section class="sf-voorbeeld">
          <h4 class="sf-voorbeeld-kop">Onder huidig recht</h4>
          <dl v-if="voorbeeld" class="sf-voorbeeld-raster">
            <div>
              <dt>Maandbedrag</dt>
              <dd>{{ euro(voorbeeld.maandbedrag) }}</dd>
            </div>
            <div>
              <dt>Klaar met betalen</dt>
              <dd>{{ voorbeeld.einde }}</dd>
            </div>
            <div>
              <dt>Hoeft niet te betalen</dt>
              <dd>{{ euroWhole(voorbeeld.kwijt) }}</dd>
            </div>
          </dl>
          <p v-else class="sf-hint">Vul een schuld en een inkomen in.</p>
        </section>

        <nldd-banner v-if="fout" variant="critical">{{ fout }}</nldd-banner>

        <div class="sf-acties">
          <nldd-button
            :text="bewerken ? 'Wijzigingen opslaan' : 'Scenario toevoegen'"
            start-icon="checked"
            appearance="primary"
            :disabled="!kanOpslaan ? true : undefined"
            @click="opslaan"
          ></nldd-button>
          <nldd-button text="Annuleren" appearance="neutral-transparent" @click="emit('close')"></nldd-button>
          <nldd-button
            v-if="kanOpslaan"
            text="Kopieer als YAML"
            start-icon="code"
            appearance="neutral-transparent"
            @click="kopieer"
          ></nldd-button>
        </div>
        <p v-if="gekopieerd" class="sf-hint">
          Gekopieerd. Plak dit in <code>data/personas.yaml</code> om het scenario te bewaren voor iedereen.
        </p>
      </nldd-container>
    </nldd-page>
  </nldd-sheet>
</template>

<!--
  Een scenario samenstellen en meteen doorrekenen. Het record dat hieruit komt
  heeft exact het schema van data/personas.yaml, en dus van een populatie-
  record: dezelfde velden, bedragen in eurocent. Daardoor kan de simulatie er
  zonder omweg mee rekenen en kun je het zo in de YAML plakken.
-->

<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import { runSimulation } from '../../composables/useSimulation.js';
import { euro, euroWhole, regimeLabel, regimeColor } from '../../lib/format.js';
import { regimeVanCohort, cohortUitleg } from '../../lib/regimeFacts.js';
import { scenarioRecord } from '../../lib/scenarioRecord.js';

const props = defineProps({
  open: { type: Boolean, default: false },
  /** Bestaand scenario om aan te passen; leeg voor een nieuw scenario. */
  scenario: { type: Object, default: null },
});
const emit = defineEmits(['close', 'opslaan']);

const HUISHOUD = {
  alleenstaand: 'Alleenstaand',
  alleenstaand_met_kind: 'Alleenstaand met kind',
  paar: 'Samenwonend of getrouwd',
  paar_met_kind: 'Samenwonend of getrouwd, met kind',
};
const ONDERWIJS = { mbo: 'Mbo', hbo: 'Hbo', wo: 'Universiteit' };

function leeg() {
  return {
    naam: '',
    schuld: 20000,
    inkomen: 32000,
    partnerinkomen: 0,
    huishoudtype: 'alleenstaand',
    startjaar: 2016,
    onderwijssoort: 'hbo',
    lllk: false,
    geboortejaar: 1995,
    groei: 2,
  };
}

const vorm = ref(leeg());
const fout = ref('');
const gekopieerd = ref(false);
const sheetEl = ref(null);

const bewerken = computed(() => !!props.scenario?.eigen);

// Het formulier vullen bij openen: leeg voor nieuw, of de waarden van het
// scenario dat je aanpast (terug van eurocent naar hele euro's).
watch(() => [props.open, props.scenario], ([open]) => {
  if (!open) return;
  fout.value = '';
  gekopieerd.value = false;
  const s = props.scenario;
  vorm.value = s
    ? {
      naam: s.naam,
      schuld: Math.round(s.schuld / 100),
      inkomen: Math.round(s.inkomen / 100),
      partnerinkomen: Math.round((s.partnerinkomen ?? 0) / 100),
      huishoudtype: s.huishoudtype,
      startjaar: Number(String(s.eerste_studiefinanciering).slice(0, 4)),
      onderwijssoort: s.onderwijssoort,
      lllk: !!s.is_levenlanglerenkrediet,
      geboortejaar: s.geboortejaar,
      groei: Math.round((s.inkomensgroei ?? 0) * 1000) / 10,
    }
    : leeg();
}, { immediate: true });

// De sheet openen en sluiten via zijn eigen API.
watch(() => props.open, async (open) => {
  await nextTick();
  const el = sheetEl.value;
  if (!el) return;
  if (open) el.show?.();
  else el.hide?.();
});

const heeftPartner = computed(() => vorm.value.huishoudtype.startsWith('paar'));

/** Het formulier als populatierecord, precies het schema uit personas.yaml. */
const alsRecord = computed(() => scenarioRecord(vorm.value, props.scenario?.bsn ?? `eigen-${Date.now()}`));

const regime = computed(() => regimeVanCohort(alsRecord.value));

const kanOpslaan = computed(
  () => vorm.value.naam.trim().length > 0 && vorm.value.schuld > 0,
);

/** Doorrekening onder huidig recht, zodat je ziet wat je hebt samengesteld. */
const voorbeeld = computed(() => {
  if (vorm.value.schuld <= 0) return null;
  try {
    const r = runSimulation(alsRecord.value, {
      overstapAangevraagd: false,
      draagkrachtAangevraagd: false,
      peiljaarverlegging: false,
      jokerMaanden: 0,
      jokerVanafMaand: 0,
      partnerMeetellen: true,
    });
    if (!r) return null;
    return {
      maandbedrag: r.timeline[0]?.maandbedrag ?? 0,
      einde: r.totals.levenslang ? 'levenslang' : String(r.totals.eindejaar),
      kwijt: r.totals.kwijtgescholden,
    };
  } catch {
    return null;
  }
});

function num(event, huidig) {
  const v = event.detail?.value ?? Number(event.target?.value);
  return v === null || v === undefined || Number.isNaN(v) ? huidig : Number(v);
}

function opslaan() {
  if (!kanOpslaan.value) {
    fout.value = 'Vul in elk geval een naam en een schuld in.';
    return;
  }
  emit('opslaan', { ...alsRecord.value });
}

/** Als YAML-fragment, klaar om in data/personas.yaml te plakken. */
async function kopieer() {
  const r = alsRecord.value;
  const yaml = [
    `  - bsn: '${r.bsn}'`,
    `    naam: ${r.naam}`,
    `    geboortejaar: ${r.geboortejaar}`,
    '    omschrijving: >-',
    '      Zelf toegevoegd scenario.',
    `    huishoudtype: ${r.huishoudtype}`,
    `    heeft_partner: ${r.heeft_partner}`,
    `    partnerinkomen: ${r.partnerinkomen}`,
    `    inkomen: ${r.inkomen}`,
    `    inkomensgroei: ${r.inkomensgroei}`,
    `    schuld: ${r.schuld}`,
    `    eerste_studiefinanciering: '${r.eerste_studiefinanciering}'`,
    `    onderwijssoort: ${r.onderwijssoort}`,
    `    is_levenlanglerenkrediet: ${r.is_levenlanglerenkrediet}`,
    '    keuzemomenten: []',
  ].join('\n');
  try {
    await navigator.clipboard.writeText(yaml);
    gekopieerd.value = true;
  } catch {
    fout.value = 'Kopiëren lukte niet; selecteer het scenario handmatig.';
  }
}
</script>

<style scoped>
.sf-velden { display: flex; flex-direction: column; gap: var(--primitives-space-16); }
.sf-rij { display: flex; gap: var(--primitives-space-16); flex-wrap: wrap; }
.sf-rij > * { flex: 1 1 200px; }
.sf-regime {
  display: flex;
  align-items: center;
  gap: var(--primitives-space-8);
  flex-wrap: wrap;
  padding: var(--primitives-space-12);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-tinted-background-color);
}
.sf-regime-label { font-weight: 600; font-size: 0.9em; }
.sf-regime-uitleg { flex: 1 1 100%; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.sf-meer {
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  padding: var(--primitives-space-8) var(--primitives-space-12);
}
.sf-meer summary { cursor: pointer; font-weight: 600; font-size: 0.9em; }
.sf-meer-velden { margin-top: var(--primitives-space-16); }
.sf-voorbeeld {
  padding: var(--primitives-space-16);
  border: 1px solid var(--semantics-dividers-color);
  border-left: 4px solid var(--semantics-content-accent-color);
  border-radius: var(--semantics-surfaces-corner-radius);
}
.sf-voorbeeld-kop { margin: 0 0 var(--primitives-space-12); font-size: 1em; font-weight: 600; }
.sf-voorbeeld-raster {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
  gap: var(--primitives-space-16);
  margin: 0;
}
.sf-voorbeeld-raster dt { font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.sf-voorbeeld-raster dd { margin: 0; font-size: 1.2em; font-weight: 700; font-variant-numeric: tabular-nums; }
.sf-acties { display: flex; gap: var(--primitives-space-8); flex-wrap: wrap; }
.sf-hint { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
</style>
