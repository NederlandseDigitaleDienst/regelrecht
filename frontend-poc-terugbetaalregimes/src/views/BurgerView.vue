<template>
  <nldd-page>
    <nldd-simple-section>
      <div class="intro-kop">
        <nldd-title :size="1">
          <span slot="overline">Voor burgers</span>
          <span>Jouw studieschuld</span>
        </nldd-title>
        <persona-switcher
          v-if="ready && personas.length"
          class="persona-switcher"
          :personas="personas"
          :selected-bsn="selected?.bsn"
          @select="selectPersona"
        />
      </div>

      <div class="meldingen">
        <nldd-banner v-if="initError" variant="critical">
          De rekenmachine kon niet worden geladen: {{ initError.message }}
        </nldd-banner>
        <nldd-banner v-else-if="!ready" variant="accent">Een moment, we halen je gegevens op…</nldd-banner>
        <nldd-banner v-if="werkversie || changeCount" variant="warning">
          Je kijkt naar een mogelijke nieuwe regeling: {{ werkversieLabel }}{{ changeCount ? ` met ${changeCount} bewerkt${changeCount === 1 ? '' : 'e'} document${changeCount === 1 ? '' : 'en'}` : '' }}. Huidig recht kies je in de balk bovenaan.
        </nldd-banner>
      </div>
    </nldd-simple-section>

    <template v-if="ready && selected && result">
      <!-- Hero-antwoord -->
      <nldd-simple-section>
        <nldd-container layout="stack" gap="16">
          <hero-antwoord
            :maandbedrag="nu"
            :onderregel="onderregel"
            :regime="regime"
          >
            <!-- Proactief signaal: hoogstens één, alleen als het waar is -->
            <nldd-banner v-if="signaal" :variant="signaal.variant">
              {{ signaal.tekst }}
            </nldd-banner>

            <!-- De onderbouwing hoort bij het bedrag dat hij verklaart, dus in
                 dezelfde kaart. Ingeklapt, want je hoeft hem niet elke keer te
                 zien; wie wil weten waaróm hij dit betaalt, klapt hem open. -->
            <details class="gegevens">
              <summary>
                <span class="gegevens-kop">Waar dit bedrag op gebaseerd is</span>
                <span class="gegevens-sub">je schuld, wat je al betaalde en het inkomen waarmee DUO rekent</span>
              </summary>
              <gegevens-blok
                class="sectie-inhoud"
                :persona="selected"
                :totals="result.totals"
                :choices="choices"
              />
            </details>
          </hero-antwoord>
        </nldd-container>
      </nldd-simple-section>

      <!-- Wat je zou kunnen aanpassen; niets doen is er altijd één van -->
      <nldd-simple-section background="tinted">
        <nldd-title :size="3">
          <span>Dit zou je kunnen aanpassen</span>
          <span slot="supporting-text">
            Je hoeft niets te doen. Laat je alles staan, dan blijft het bedrag hierboven gelden. Zet hieronder iets
            aan om te zien wat er dan verandert.
          </span>
        </nldd-title>
        <div class="keuze-grid">
          <keuze-vraag
            v-for="choice in zinvolleKeuzes"
            :key="choice"
            :persona="selected"
            :choice="choice"
            :base-choices="choices"
            @update="onChoiceUpdate"
            @zinvol="onZinvol"
          />
        </div>

        <!-- Keuzes die nu niets opleveren: als regel, niet als knop. -->
        <div v-if="overigeKeuzes.length" class="keuze-overig">
          <p class="keuze-overig-kop">Dit kan ook, maar het helpt je nu niet:</p>
          <keuze-vraag
            v-for="choice in overigeKeuzes"
            :key="choice"
            :persona="selected"
            :choice="choice"
            :base-choices="choices"
            @update="onChoiceUpdate"
            @zinvol="onZinvol"
          />
        </div>

        <!-- Je inkomen wijzigen is voor een burger net zo goed "iets aanpassen"
             als overstappen naar andere regels. Ingeklapt, want het is pas
             relevant als je inkomen daadwerkelijk verandert. -->
        <details class="aanpas-blok">
          <summary>
            <span class="aanpas-kop">Verandert je inkomen?</span>
            <span class="aanpas-sub">ga je de komende jaren meer of minder verdienen? kijk wat het effect is</span>
          </summary>
          <inkomen-wat-als
            class="sectie-inhoud"
            :persona="selected"
            :choices="choices"
            @wijzigingen="onInkomenWijzigingen"
          />
        </details>

        <!-- Eén mand voor alles wat je op deze pagina aanpast, vormgegeven als
             een bonnetje: dit is wat je straks verstuurt, dus het moet eruit
             springen en niet lezen als nog een informatieblok. -->
        <section v-if="regels.length" class="bon" aria-labelledby="bon-kop">
          <header class="bon-kop">
            <h4 id="bon-kop">Dit ga je versturen</h4>
            <nldd-badge size="sm" color="accent" :number="regels.length" accessible-label="aantal wijzigingen"></nldd-badge>
          </header>

          <ol class="bon-regels">
            <li v-for="(r, i) in regels" :key="r.tekst" class="bon-regel">
              <span class="bon-nr">{{ i + 1 }}</span>
              <span class="bon-inhoud">
                <span class="bon-tekst">{{ r.tekst }}</span>
                <span class="bon-soort">{{ r.soort === 'doorgeven' ? 'je geeft dit door' : 'je vraagt dit aan' }}</span>
              </span>
            </li>
          </ol>

          <div v-if="nu !== null" class="bon-totaal">
            <span class="bon-totaal-label">Je maandbedrag wordt</span>
            <strong class="bon-totaal-waarde">{{ euro(nu) }}</strong>
          </div>

          <p class="bon-hint">
            Een wijziging verwerkt DUO meteen. Een aanvraag bekijkt DUO eerst. Daar krijg je een brief over.
            Zolang je dit niet verstuurt, verandert er niets.
          </p>

          <nldd-button
            class="bon-knop"
            :text="verstuurTekst"
            start-icon="send"
            appearance="primary"
            @click="aanvraagOpen = true"
          ></nldd-button>
        </section>

        <nldd-banner v-if="aanvraagOpen" variant="warning">
          Dit is een demo. Er gaat niets naar DUO. In het echt geef je een wijziging door in Mijn DUO. Een aanvraag
          doe je met een formulier. Daarna krijg je een brief. Ben je het er niet mee eens? Dan kun je bezwaar maken.
        </nldd-banner>
      </nldd-simple-section>

      <!-- De uitkomst staat niet op zichzelf -->
      <nldd-simple-section>
        <nldd-title :size="3">
          <span>Wat dit nog meer raakt</span>
          <span slot="supporting-text">Andere regelingen kijken naar hetzelfde inkomen.</span>
        </nldd-title>
        <toeslagen-signaal class="sectie-inhoud" :persona="selected" :choices="choices" />
      </nldd-simple-section>

      <!-- Rustige detailregel onderaan -->
      <nldd-simple-section>
        <div class="detail-rij">
          <nldd-button
            text="Bekijk hoe dit is berekend"
            start-icon="text-document"
            appearance="secondary"
            @click="openBerekening"
          ></nldd-button>
          <span class="demo-label">Demo: wat als het beleid verandert? Kies bovenaan een werkversie; deze pagina rekent ermee.</span>
        </div>
      </nldd-simple-section>
    </template>

    <berekening-sheet
      :open="berekeningOpen"
      :timeline="result?.timeline ?? []"
      :trace="trace"
      @close="berekeningOpen = false"
    />
  </nldd-page>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue';
import { useEngine } from '../engine/useEngine.js';
import { useLawStore } from '../engine/lawStore.js';
import { usePersonas } from '../composables/usePersonas.js';
import { runSimulation, traceOutput, defaultChoices } from '../composables/useSimulation.js';
import HeroAntwoord from '../components/burger/HeroAntwoord.vue';
import KeuzeVraag from '../components/burger/KeuzeVraag.vue';
import BerekeningSheet from '../components/burger/BerekeningSheet.vue';
import PersonaSwitcher from '../components/burger/PersonaSwitcher.vue';
import GegevensBlok from '../components/burger/GegevensBlok.vue';
import InkomenWatAls from '../components/burger/InkomenWatAls.vue';
import ToeslagenSignaal from '../components/burger/ToeslagenSignaal.vue';
import RegimeTabel from '../components/burger/RegimeTabel.vue';
import { euro } from '../lib/format.js';
import { writePersisted, readPersisted } from '../composables/useBewaardeStand.js';
import { budgetSignaal, heroOnderregel } from '../lib/burgerTekst.js';
import { isPersonaWissel, keuzesBijBinnenkomst } from '../lib/personaKeuze.js';

const { ready, initError, lawIndex, initEngine } = useEngine();
const { initStore, version, werkversie, werkversieLabel, changeCount } = useLawStore();
const { personas, fetchPersonas } = usePersonas();

const selected = ref(null);
const choices = ref({});
const berekeningOpen = ref(false);
const aanvraagOpen = ref(false);
const trace = ref(null);

onMounted(async () => {
  await fetchPersonas().catch(() => {});
  try {
    const engine = await initEngine();
    await initStore(engine, lawIndex.value);
    // "Ingelogd" als de eerste persona; wisselen kan via de DigiD-switcher.
    // Was je vorige keer iemand anders, dan kom je daar weer terug, inclusief
    // de keuzes die je toen had aangezet.
    if (!selected.value && personas.value.length) {
      const bewaardBsn = readPersisted('burger.bsn', null);
      const eerder = bewaardBsn && personas.value.find((p) => p.bsn === bewaardBsn);
      selectPersona(eerder ?? personas.value[0]);
    }
  } catch {
    // fout via initError
  }
});

function selectPersona(persona) {
  // Alleen een échte wissel telt als "nieuwe situatie" (zie isPersonaWissel).
  if (!isPersonaWissel(selected.value, persona)) return;

  selected.value = persona;
  writePersisted('burger.bsn', persona.bsn ?? null);
  // Keuzes die je eerder bij deze persoon aanzette komen terug; bij iemand
  // anders begin je met diens eigen uitgangspunt.
  const bewaard = readPersisted(`burger.keuzes.${persona.bsn}`, null);
  choices.value = keuzesBijBinnenkomst(defaultChoices(persona), bewaard);
}

/**
 * De keuzes die deze persoon kan maken. De persona-lijst is het vertrekpunt
 * (wat voor deze demo interessant is), maar een keuze die de wet nú toestaat
 * hoort er sowieso bij te staan: peiljaarverlegging wordt pas mogelijk bij
 * een daling van meer dan 15 procent (artikel 6.12), en die situatie kun je
 * op deze pagina zelf maken met de inkomensschuif.
 */
const keuzemomenten = computed(() => {
  const lijst = [...(selected.value?.keuzemomenten ?? [])];
  const eerste = result.value?.timeline?.[0];
  if (eerste?.peiljaarverlegging_mogelijk && !lijst.includes('peiljaarverlegging')) {
    lijst.push('peiljaarverlegging');
  }
  return lijst;
});

const result = computed(() => {
  version.value;
  if (!selected.value) return null;
  return runSimulation(selected.value, choices.value);
});

const nu = computed(() => (result.value?.timeline.length ? result.value.timeline[0].maandbedrag : null));
const regime = computed(() => result.value?.totals?.regime ?? null);
const onderregel = computed(() =>
  result.value ? heroOnderregel(result.value.totals, choices.value) : '',
);

/**
 * Proactief signaal: hoogstens één banner, en alleen wanneer het waar is.
 * 1. Je kunt waarschijnlijk minder betalen (draagkrachtmeting nog niet gedaan,
 *    en die zou het maandbedrag verlagen).
 * 2. Je inkomen komt niet uit boven het Nibud-referentiebudget, dus élk bedrag
 *    knelt. De werkinstructie zet de afloscapaciteit dan op nul; dat is iets
 *    anders dan "dit bedrag is te hoog", en hoort ook anders te klinken.
 * 3. Het maandbedrag ligt boven wat er volgens Nibud aan ruimte is.
 */
const signaal = computed(() => {
  version.value;
  if (!result.value) return null;
  const t0 = result.value.timeline[0];
  if (!t0) return null;

  const draagkrachtMogelijk = keuzemomenten.value.includes('draagkrachtmeting');
  const draagkrachtNietGedaan = !choices.value.draagkrachtAangevraagd;

  // Vergelijk het bedrag mét meting met wat je nu al betaalt; alleen tonen als
  // een meting je maandbedrag echt verlaagt (niet als het gelijk blijft).
  if (draagkrachtMogelijk && draagkrachtNietGedaan) {
    const metMeting = runSimulation(selected.value, { ...choices.value, draagkrachtAangevraagd: true });
    const bedragMet = metMeting?.timeline?.[0]?.maandbedrag;
    if (bedragMet !== null && bedragMet !== undefined && bedragMet < t0.maandbedrag) {
      return {
        variant: 'accent',
        tekst: `Je kunt waarschijnlijk minder betalen: met een draagkrachtmeting wordt je maandbedrag ${euro(bedragMet)}. Dit vraag je aan bij DUO.`,
      };
    }
  }

  return budgetSignaal(t0);
});

/** Wat de burger heeft aangezet, in de woorden van de wet ("een aanvraag indienen"). */
const AANVRAAG_TEKST = {
  overstapAangevraagd: 'Overstappen naar de nieuwe terugbetaalregels',
  draagkrachtAangevraagd: 'Je draagkracht laten vaststellen, zodat je maandbedrag bij je inkomen past',
  peiljaarverlegging: 'Peiljaarverlegging: rekenen met een recenter inkomensjaar',
};

const aangezet = computed(() => {
  const basis = defaultChoices(selected.value ?? {});
  const uit = [];
  for (const [sleutel, tekst] of Object.entries(AANVRAAG_TEKST)) {
    if (choices.value[sleutel] && !basis[sleutel]) uit.push(tekst);
  }
  // Jokerjaren en de partner-opt-out zijn ook aanvragen, maar met een eigen tekst.
  if ((choices.value.jokerMaanden ?? 0) > (basis.jokerMaanden ?? 0)) {
    uit.push(`Een aflospauze van ${Math.round(choices.value.jokerMaanden / 12)} jaar (jokerjaren)`);
  }
  if (choices.value.partnerMeetellen === false && basis.partnerMeetellen !== false) {
    uit.push('Het inkomen van je partner niet laten meetellen');
  }
  return uit;
});

// Elke kaart meldt of hij op dit moment iets oplevert; zo staan de bruikbare
// keuzes in het raster en de rest als regel eronder.
const zinvolPerKeuze = ref({});
function onZinvol(choice, zinvol) {
  zinvolPerKeuze.value = { ...zinvolPerKeuze.value, [choice]: zinvol };
}
const zinvolleKeuzes = computed(() => keuzemomenten.value.filter((c) => zinvolPerKeuze.value[c] !== false));
const overigeKeuzes = computed(() => keuzemomenten.value.filter((c) => zinvolPerKeuze.value[c] === false));

// Wat het inkomensblok wil doorgeven of aanvragen; dat komt uit dat blok,
// want daar staan de schuiven en de schakelaars.
const inkomenWijzigingen = ref({ doorgeven: [], aanvragen: [] });
function onInkomenWijzigingen(w) {
  inkomenWijzigingen.value = w;
}

/**
 * Eén mand voor alles wat je op deze pagina aanpast: de keuzes bovenaan en
 * het inkomensblok eronder. Een wijziging geef je door (DUO verwerkt hem),
 * een aanvraag beoordeelt DUO; dat onderscheid blijft, de knop niet.
 */
const teDoen = computed(() => ({
  doorgeven: inkomenWijzigingen.value.doorgeven ?? [],
  aanvragen: [...aangezet.value, ...(inkomenWijzigingen.value.aanvragen ?? [])],
}));

/** Alles wat in de mand zit, als één genummerde lijst met het soort erbij. */
const regels = computed(() => [
  ...teDoen.value.doorgeven.map((tekst) => ({ tekst, soort: 'doorgeven' })),
  ...teDoen.value.aanvragen.map((tekst) => ({ tekst, soort: 'aanvragen' })),
]);

const verstuurTekst = computed(() => {
  const heeftDoor = teDoen.value.doorgeven.length > 0;
  const heeftAan = teDoen.value.aanvragen.length > 0;
  if (heeftDoor && heeftAan) return 'Doorgeven en aanvragen bij DUO';
  if (heeftAan) return teDoen.value.aanvragen.length === 1 ? 'Aanvraag indienen bij DUO' : 'Aanvragen indienen bij DUO';
  return 'Inkomen doorgeven aan DUO';
});

function onChoiceUpdate(newChoices) {
  choices.value = newChoices;
  if (selected.value) writePersisted(`burger.keuzes.${selected.value.bsn}`, newChoices);
  aanvraagOpen.value = false; // een nieuwe keuze vraagt om een nieuwe bevestiging
}

function openBerekening() {
  trace.value = traceOutput(selected.value, choices.value);
  berekeningOpen.value = true;
}

// Houd de trace in het geopende paneel actueel bij keuze- of wetswijziging.
watch([choices, version], () => {
  if (berekeningOpen.value) trace.value = traceOutput(selected.value, choices.value);
});
</script>

<style scoped>
.meldingen { display: flex; flex-direction: column; gap: var(--primitives-space-8); margin-top: var(--primitives-space-16); }
.meldingen:empty { display: none; }
.intro-kop {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--primitives-space-16);
  flex-wrap: wrap;
}
.persona-switcher { margin-left: auto; }
/* Alleen ruimte onder de sectiekop; de component bepaalt zelf zijn display.
   Met `display: block` verviel de flex-layout binnenin en plakten de blokken
   tegen elkaar. */
.sectie-inhoud { margin-top: var(--primitives-space-16); }
.aanpas-blok {
  margin-top: var(--primitives-space-16);
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  padding: var(--primitives-space-12) var(--primitives-space-16);
  background: var(--semantics-surfaces-base-background-color);
}
.aanpas-blok summary { cursor: pointer; }
.aanpas-kop { font-weight: 600; }
.aanpas-sub { color: var(--semantics-content-secondary-color); }
.aanpas-sub::before { content: ' · '; }
.gegevens {
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  padding: var(--primitives-space-12) var(--primitives-space-16);
}
.gegevens summary { cursor: pointer; }
.gegevens-kop { font-weight: 600; }
.gegevens-sub { color: var(--semantics-content-secondary-color); }
.gegevens-sub::before { content: ' · '; }
.regimes {
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  padding: var(--primitives-space-8) var(--primitives-space-12);
}
.regimes summary { cursor: pointer; font-weight: 600; font-size: 0.9em; }
.regimes-tabel { display: block; margin-top: var(--primitives-space-12); }

/* Het bonnetje: dit is wat je verstuurt, dus het mag opvallen. Accentrand
   links en een eigen achtergrond zetten het apart van de informatieblokken
   eromheen. Eén kolom, ook op desktop: een bon lees je van boven naar
   beneden, en de knop hoort onderaan na wat je verstuurt. */
.bon {
  margin-top: var(--primitives-space-16);
  border: 1px solid var(--semantics-dividers-color);
  border-left: 4px solid var(--semantics-content-accent-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  padding: var(--primitives-space-16);
  background: var(--semantics-surfaces-base-background-color);
  display: flex;
  flex-direction: column;
  gap: var(--primitives-space-12);
}
.bon-kop { display: flex; align-items: center; gap: var(--primitives-space-8); }
.bon-kop h4 { margin: 0; font-size: 1.05em; }
.bon-regels { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; }
.bon-regel {
  display: flex;
  gap: var(--primitives-space-12);
  align-items: baseline;
  padding: var(--primitives-space-8) 0;
  border-bottom: 1px dashed var(--semantics-dividers-color);
}
.bon-regel:first-child { padding-top: 0; }
/* Streepjeslijn tussen de regels, niet eronder: dat is de bonnetjes-look. */
.bon-regel:last-child { border-bottom: none; padding-bottom: 0; }
.bon-nr {
  flex: 0 0 auto;
  min-width: 1.5rem;
  height: 1.5rem;
  border-radius: 50%;
  background: var(--semantics-surfaces-tinted-background-color);
  color: var(--semantics-content-secondary-color);
  font-size: 0.8em;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.bon-inhoud { display: flex; flex-direction: column; gap: 2px; }
.bon-tekst { font-weight: 600; }
.bon-soort { font-size: 0.8em; color: var(--semantics-content-secondary-color); }
/* De uitkomst, zoals het totaal onderaan een bon. */
.bon-totaal {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--primitives-space-12);
  padding-top: var(--primitives-space-12);
  border-top: 1px solid var(--semantics-dividers-color);
}
.bon-totaal-label { color: var(--semantics-content-secondary-color); }
.bon-totaal-waarde { font-size: 1.3em; font-variant-numeric: tabular-nums; }
.bon-hint { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
/* Volle breedte op mobiel, links uitgelijnd zodra er ruimte is. */
.bon-knop { align-self: stretch; }
@media (min-width: 480px) { .bon-knop { align-self: flex-start; } }

.keuze-grid {
  display: grid;
  /* auto-fit: blijft er één kaart over, dan mag die de volle breedte pakken. */
  grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
  gap: var(--primitives-space-16);
  margin-top: var(--primitives-space-16);
}
.keuze-grid:empty { display: none; }
.keuze-overig {
  display: flex;
  flex-direction: column;
  gap: var(--primitives-space-4);
  margin-top: var(--primitives-space-16);
}
.keuze-overig-kop { margin: 0; font-size: 0.85em; font-weight: 600; color: var(--semantics-content-secondary-color); }
.detail-rij {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--primitives-space-16);
  flex-wrap: wrap;
}
.demo-label {
  font-size: 0.8em;
  color: var(--semantics-content-secondary-color);
}
</style>
