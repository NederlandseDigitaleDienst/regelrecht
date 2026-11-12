<script setup lang="ts">
/**
 * AddLawSheet - "Regelwerk toevoegen" vanuit een traject, in één flow, gepresenteerd
 * als sheet (rechts; op klein scherm een bottom-sheet). Vervangt de eerdere
 * AddLawPopover, die de combobox-listbox van SearchPopover overnam voor iets
 * dat geen zoek-in-place maar een eigen taak is.
 *
 * 1. Zoek in het centrale corpus (op naam en law-id) via dezelfde
 *    traject-scoped zoek-API die de bibliotheekzoeker voedt - het traject
 *    federeert de centrale corpus-seed, dus die zoekt door het volledige
 *    centrale corpus en markeert wat al in de eigen repo staat
 *    (source_priority 0). Geen tweede zoekpad.
 * 2. Gevonden? "Toevoegen aan traject" kopieert de volledige wet-map naar de
 *    traject-repo via de promote-endpoint. Staat de wet al in het traject,
 *    dan is de knop uitgeschakeld ("Al in dit traject").
 * 3. Niet gevonden? Met een BWB-id (direct getypt, of via de
 *    wetten.overheid.nl-zoeker) start "Ophalen naar traject" een
 *    traject-scoped harvest via het taken-mechanisme; de aanvraag is direct
 *    zichtbaar in het takenpaneel en het resultaat komt terug als review-taak.
 * 4. Staat de wet nergens (of alleen op papier)? Upload een PDF/DOCX; de
 *    conversie-naar-wet-keten levert het resultaat als review-taak.
 *
 * De sheet-opzet (nldd-sheet > nldd-page sticky-header > nldd-top-title-bar,
 * nldd-simple-section, footer) spiegelt EditSheet.vue en NewHarvestJobSheet.vue.
 */
import { ref, computed, watch, nextTick } from 'vue';
import { storeToRefs } from 'pinia';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import type { NLDDTextField } from '@nldd/design-system/text-field';
import { useBwbSearch, MIN_QUERY_LENGTH } from '../composables/useBwbSearch';
import { useCorpus, type CorpusLaw } from '../corpus';
import { useTrajectsStore } from '../stores/trajects';
import { useLawPromote } from '../composables/useLawPromote';
import { trajectHarvestUrl } from '../composables/corpusUrls';
import { apiFetch, ApiError } from '../lib/apiFetch';
import { useLatest } from '../lib/useLatest';

const emit = defineEmits<{
  promoted: [lawId: string];
  'harvest-requested': [bwbId: string];
  'upload-requested': [];
}>();

/** An nldd input event: the value is on `detail` (CustomEvent) or on the target. */
type ValueEvent = Event & { detail?: { value?: string } | null };

/** A traject-scoped harvest request's state, per bwb_id. */
type HarvestRequestState = 'busy' | 'requested' | 'conflict' | 'error';

const { activeTrajectRef, activeTraject } = storeToRefs(useTrajectsStore());
const corpus = useCorpus();
const { results: bwbResults, loading: bwbLoading, search: searchBwb, clear: clearBwb } = useBwbSearch();

const search = ref('');
const sheetRef = ref<NLDDSheet | null>(null);
const searchInputRef = ref<NLDDTextField | null>(null);

// 'search' | 'upload' - twee losse routes, gescheiden met een segmented control
// bovenin. Zonder die scheiding hing het uploaden als een full-width knop onder
// de zoeker, wat als submit-actie van het zoekformulier las.
const mode = ref<'search' | 'upload'>('search');
function onModeChange(e: Event) {
  const event = e as ValueEvent;
  const next = event.detail?.value ?? (event.target as { value?: string } | null)?.value;
  if (next === 'search' || next === 'upload') mode.value = next;
}

// Corpus-treffers (traject-gefedereerd: eigen repo + centrale seed) voor de
// huidige zoekterm.
const centralLaws = ref<CorpusLaw[]>([]);
const searching = ref(false);
const searchFailed = ref(false);

// The search is traject-scoped whenever a traject is active (see `runSearch`:
// it hits `/api/trajects/{ref}/corpus/laws`), so a failure is about *this
// traject's* library - its own repo could not be scanned - not the central
// corpus. What decides the scope is `activeTrajectRef` (read straight from the
// route, so it is there from the first render), NOT `activeTraject`: that one
// is looked up in the asynchronously loaded trajects list and is still null
// while the list is in flight, or when the list request itself failed. Keying
// the wording on the name alone would point the user at the central corpus for
// a request that never went there. So: name the traject when we know it, say
// "dit traject" when we only know there is one, and keep the central-corpus
// wording strictly for the no-traject search.
const searchFailedText = computed(() => {
  if (!activeTrajectRef.value) {
    return 'Het centrale corpus kon niet worden doorzocht. Probeer het opnieuw.';
  }
  const scope = activeTraject.value?.name
    ? `van ${activeTraject.value.name}`
    : 'van dit traject';
  return `De bibliotheek ${scope} kon niet worden doorzocht. Probeer het opnieuw.`;
});

// Gedeelde promote-logica (ook gebruikt door de gewone zoekresultaten in
// SearchPopover): per-wet busy-state, al-in-traject-set, 409-afhandeling.
const {
  promoteState,
  promoteError,
  setLawsFromSearch,
  isInTraject: isLawIdInTraject,
  clearPromoteError,
  promote: promoteLaw,
} = useLawPromote(activeTrajectRef);

// Per bwb_id: 'busy' | 'requested' | 'conflict' | 'error'.
const harvestState = ref<Record<string, HarvestRequestState>>({});

function displayName(law: CorpusLaw) {
  if (law.display_name) return law.display_name;
  if (law.name) return law.name;
  return law.law_id.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}

const sortedLaws = computed(() =>
  [...centralLaws.value].sort(
    (a, b) =>
      (a.source_priority ?? Number.MAX_SAFE_INTEGER) -
      (b.source_priority ?? Number.MAX_SAFE_INTEGER),
  ),
);

// Direct getypt BWB-id: bied ophalen aan zonder dat de externe zoeker het
// hoeft te vinden (acceptatie: "een BWB-id mag ook").
const bwbIdQuery = computed(() => {
  const term = search.value.trim().toUpperCase();
  return /^BWB[A-Z0-9]{3,17}$/.test(term) ? term : null;
});

// De directe rij alleen tonen zolang de externe zoeker hetzelfde id niet al
// als resultaat (met titel) toont - anders staan er twee rijen voor dezelfde
// wet met dezelfde harvest-knop.
const showDirectBwbRow = computed(
  () =>
    bwbIdQuery.value &&
    sortedLaws.value.length === 0 &&
    !bwbResults.value.some((r) => r.bwb_id === bwbIdQuery.value),
);

// Zijn er zichtbare resultaatrijen? Zo niet, dan tonen we een van de
// begeleidende toestanden (zoeken/mislukt/leeg) in plaats van de lijst.
const hasRows = computed(
  () => sortedLaws.value.length > 0 || showDirectBwbRow.value || bwbResults.value.length > 0,
);

const claimSearch = useLatest();
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

watch(search, (q) => {
  clearBwb();
  searchFailed.value = false;
  clearPromoteError();
  if (debounceTimer) clearTimeout(debounceTimer);
  const term = q.trim();
  if (term.length < MIN_QUERY_LENGTH) {
    claimSearch();
    centralLaws.value = [];
    searching.value = false;
    return;
  }
  searching.value = true;
  debounceTimer = setTimeout(() => runSearch(term), 200);
});

async function runSearch(term: string) {
  const isCurrent = claimSearch();
  try {
    // Traject-scoped zoekopdracht: dekt de volledige centrale seed (het
    // traject federeert die) en de eigen repo. source_priority 0 = de eigen
    // schrijfbare repo -> al in het traject, niet promoteerbaar.
    const { laws } = await corpus.searchLaws({ q: term, limit: 60 });
    if (!isCurrent()) return;
    centralLaws.value = laws;
    setLawsFromSearch(laws);
    searchFailed.value = false;
    // Niets in het centrale corpus: val terug op wetten.overheid.nl zodat de
    // gebruiker met een BWB-id een traject-harvest kan starten.
    if (laws.length === 0) searchBwb(term);
  } catch {
    if (isCurrent()) {
      centralLaws.value = [];
      searchFailed.value = true;
    }
  } finally {
    if (isCurrent()) searching.value = false;
  }
}

function isInTraject(law: CorpusLaw) {
  return isLawIdInTraject(law.law_id);
}

async function promote(law: CorpusLaw) {
  const outcome = await promoteLaw(law.law_id);
  if (outcome === 'done') {
    emit('promoted', law.law_id);
    close();
  }
}

async function requestTrajectHarvest(bwbId: string, lawName?: string) {
  if (!activeTrajectRef.value) return;
  const state = harvestState.value[bwbId];
  if (state === 'busy' || state === 'requested') return;
  harvestState.value = { ...harvestState.value, [bwbId]: 'busy' };
  try {
    await apiFetch(trajectHarvestUrl(activeTrajectRef.value), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bwb_id: bwbId, law_name: lawName || undefined }),
    });
    harvestState.value = { ...harvestState.value, [bwbId]: 'requested' };
    emit('harvest-requested', bwbId);
  } catch (e) {
    harvestState.value = {
      ...harvestState.value,
      [bwbId]: e instanceof ApiError && e.status === 409 ? 'conflict' : 'error',
    };
  }
}

function harvestStatusText(bwbId: string, fallback?: string) {
  switch (harvestState.value[bwbId]) {
    case 'busy':
      return 'Aanvragen…';
    case 'requested':
      return 'Aanvraag gestart — volg de voortgang bij Taken';
    case 'conflict':
      return 'Er loopt al een aanvraag voor deze wet';
    case 'error':
      return 'Aanvragen mislukt — probeer het opnieuw';
    default:
      return fallback || '';
  }
}

function close() {
  // `?.` op hide zelf: in tests (happy-dom) is het custom element niet
  // geüpgraded en bestaat de methode niet.
  sheetRef.value?.hide?.();
}

// Vierde route naast promoten en harvest: een PDF/Word-document uploaden dat de
// conversie-naar-wet-keten start. De file-picker zelf leeft in LibraryView;
// eerst sluiten, anders opent de picker achter de sheet.
function requestUpload() {
  close();
  emit('upload-requested');
}

function onSearchInput(e: Event) {
  const event = e as ValueEvent;
  search.value = event.detail?.value ?? (event.target as { value?: string } | null)?.value ?? '';
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && !e.defaultPrevented) close();
}

function onSheetClose() {
  search.value = '';
  mode.value = 'search';
  clearBwb();
  clearPromoteError();
}

/** Public API: open de sheet. De anchor uit de popover-tijd wordt genegeerd -
 *  een sheet plaatst zichzelf. */
async function show() {
  if (!sheetRef.value) return;
  sheetRef.value.show();
  await nextTick();
  searchInputRef.value?.focus?.();
}

defineExpose({ show });
</script>

<template>
  <!-- Geteleporteerd naar body door de aanroepende view (LibraryView), net als
       SearchPopover; een overlay die als split-view-sibling blijft staan erft
       ::slotted flex-grow en steelt paneelhoogte. -->
  <nldd-sheet
    ref="sheetRef"
    placement="right"
    width="480px"
    accessible-label="Regelwerk toevoegen aan traject"
    @close="onSheetClose"
  >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Regelwerk toevoegen"
          dismiss-text="Annuleer"
          @dismiss="close"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <nldd-segmented-control
            size="md"
            width="full"
            :value="mode"
            accessible-label="Zoeken of uploaden"
            @change="onModeChange"
          >
            <nldd-segmented-control-item value="search" text="Zoeken"></nldd-segmented-control-item>
            <nldd-segmented-control-item value="upload" text="Uploaden"></nldd-segmented-control-item>
          </nldd-segmented-control>

          <nldd-spacer size="16"></nldd-spacer>

          <!-- Route 1: zoeken in het centrale corpus + BWB-harvest. -->
          <template v-if="mode === 'search'">
          <nldd-form-field
            label="Zoek een regelwerk"
            supporting-label="Op naam, law-id of BWB-id (bijv. BWBR0002399)"
          >
            <nldd-text-field
              ref="searchInputRef"
              size="md"
              width="full"
              :value="search"
              placeholder="Zoek een regelwerk of BWB-id…"
              @input="onSearchInput"
              @keydown="onKeydown"
            ></nldd-text-field>
          </nldd-form-field>

          <nldd-spacer size="16"></nldd-spacer>

          <!-- Fout bij promoten: banner boven de resultaten. -->
          <template v-if="promoteError">
            <nldd-inline-dialog variant="alert" :text="promoteError"></nldd-inline-dialog>
            <nldd-spacer size="12"></nldd-spacer>
          </template>

          <!-- Resultaatrijen: promoten (centraal corpus) of ophalen (BWB). -->
          <nldd-list v-if="hasRows" appearance="box-tinted">
            <!-- Centrale-corpus-treffers: promoten naar het traject. -->
            <nldd-list-item
              v-for="law in sortedLaws"
              :key="law.law_id"
              size="md"
              :data-law-id="law.law_id"
            >
              <nldd-text-cell
                :text="displayName(law)"
                :supporting-text="isInTraject(law) ? 'Al in dit traject' : law.source_name || undefined"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-button
                  size="sm"
                  appearance="primary"
                  text="Toevoegen aan traject"
                  :disabled="isInTraject(law) || undefined"
                  :loading="promoteState[law.law_id] === 'busy' || undefined"
                  @click="promote(law)"
                ></nldd-button>
              </nldd-cell>
            </nldd-list-item>

            <!-- Direct getypt BWB-id: traject-harvest zonder externe zoeker. -->
            <!-- `&& bwbIdQuery` changes nothing at runtime (showDirectBwbRow already
                 requires it); it narrows bwbIdQuery to a string for the type checker. -->
            <nldd-list-item v-if="showDirectBwbRow && bwbIdQuery" size="md" :data-bwb-id="bwbIdQuery">
              <nldd-icon-cell size="20"><nldd-icon icon="harvest"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell
                :text="bwbIdQuery"
                :supporting-text="harvestStatusText(bwbIdQuery, 'Haal dit BWB-id op uit wetten.overheid.nl')"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-button
                  size="sm"
                  appearance="primary"
                  text="Ophalen naar traject"
                  :disabled="['busy', 'requested'].includes(harvestState[bwbIdQuery] ?? '') || undefined"
                  @click="requestTrajectHarvest(bwbIdQuery)"
                ></nldd-button>
              </nldd-cell>
            </nldd-list-item>

            <!-- Externe wetten.overheid.nl-treffers: traject-scoped harvest. -->
            <nldd-list-item
              v-for="result in bwbResults"
              :key="result.bwb_id"
              size="md"
              :data-bwb-id="result.bwb_id"
            >
              <nldd-icon-cell size="20"><nldd-icon icon="harvest"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell
                :text="result.title"
                :supporting-text="harvestStatusText(result.bwb_id, `${result.type} - ${result.bwb_id}`)"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-button
                  size="sm"
                  appearance="primary"
                  text="Ophalen naar traject"
                  :disabled="['busy', 'requested'].includes(harvestState[result.bwb_id] ?? '') || undefined"
                  @click="requestTrajectHarvest(result.bwb_id, result.title)"
                ></nldd-button>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>

          <!-- Toestanden zonder zichtbare resultaatrijen. -->
          <template v-else>
            <nldd-inline-dialog v-if="searching" text="Zoeken in het centrale corpus…"></nldd-inline-dialog>
            <nldd-inline-dialog
              v-else-if="searchFailed"
              variant="alert"
              text="Zoeken is mislukt"
              :supporting-text="searchFailedText"
            ></nldd-inline-dialog>
            <nldd-inline-dialog
              v-else-if="bwbLoading"
              text="Zoeken op wetten.overheid.nl..."
            ></nldd-inline-dialog>
            <nldd-inline-dialog
              v-else-if="search.length > 0 && search.length < MIN_QUERY_LENGTH"
              text="Typ minimaal drie tekens om te zoeken"
            ></nldd-inline-dialog>
            <nldd-inline-dialog
              v-else-if="search.length >= MIN_QUERY_LENGTH"
              text="Geen resultaten gevonden"
              supporting-text="Zoek op naam, law-id of BWB-id (bijv. BWBR0002399)"
            ></nldd-inline-dialog>
          </template>
          </template>

          <!-- Route 2: upload een PDF/DOCX wanneer de wet nergens (of alleen op
               papier) staat; de conversie-naar-wet-keten levert het resultaat
               als review-taak. -->
          <template v-else>
            <!-- No form field: the button is no input, it asks LibraryView
                 to open its file picker. -->
            <nldd-inline-dialog
              horizontal-alignment="left"
              text="Upload een document"
              supporting-text="PDF of Word. Bij deze conversie leest AI het document: er is geen route naar een regelwerk zonder taalmodel. De keten zet het om naar een eerste opzet van het regelwerk en verrijkt die; het resultaat komt terug als review-taak bij Taken."
            >
              <nldd-button
                slot="actions"
                appearance="secondary"
                start-icon="upload-to-cloud"
                text="Bestand kiezen…"
                data-testid="add-law-upload"
                @click="requestUpload"
              ></nldd-button>
            </nldd-inline-dialog>
          </template>
        </nldd-simple-section>
      </nldd-page>
  </nldd-sheet>
</template>
