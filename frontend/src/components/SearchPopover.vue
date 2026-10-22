<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue';
import type { NLDDPopover } from '@nldd/design-system/popover';
import { useBwbSearch, MIN_QUERY_LENGTH } from '../composables/useBwbSearch';
import type { BwbSearchResult } from '../composables/useBwbSearch';
import type { CorpusLaw } from '../composables/useCorpusLaws';
import { useBwbHarvest } from '../composables/useBwbHarvest';
import { useAuth } from '../composables/useAuth';
import { useTrajects } from '../composables/useTrajects';
import { lawsListUrl } from '../composables/corpusUrls';
import { enrichedOnly } from '../composables/useLawOverview';
import { apiFetchJson } from '../lib/apiFetch';
import { useLatest } from '../lib/useLatest';
import { SEARCH_PLACEHOLDER, SEARCH_ACCESSIBLE_LABEL } from '../constants';

// Override the list's built-in search-field placeholder (i18n default "Zoeken")
// so this popover stays in sync with the main search-field in AppShell.
const listTranslations = { 'components.list.search-placeholder-label': SEARCH_PLACEHOLDER };

const emit = defineEmits<{
  'select-law': [lawId: string];
  'harvest-available': [slug: string];
}>();

/** The listbox's own `input` event: the search value on `detail`. */
type ValueEvent = Event & { detail?: { value?: string } | null };

const { activeTrajectRef } = useTrajects();

const { results: bwbResults, loading: bwbLoading, search: searchBwb, clear: clearBwb } = useBwbSearch();
const {
  harvestStatus, harvestSlugs, hasActiveHarvests,
  requestHarvest, isAvailable, isPolling, isTerminal,
  statusText, statusIcon,
} = useBwbHarvest();
const { authenticated, oidcConfigured, login } = useAuth();

const needsLogin = computed(() => oidcConfigured.value && !authenticated.value);

const search = ref('');
// What the search actually runs on: the watch below trims before it compares
// against MIN_QUERY_LENGTH, so the hints in the template have to compare the
// same thing, or "ab " says "no results" instead of "type more".
const searchTerm = computed(() => search.value.trim());
const popoverRef = ref<NLDDPopover | null>(null);
const useCenteredPosition = ref(true);
// md only: popover anchors below the trigger button - clicking outside closes
// it, so an explicit Sluit button is just clutter. On sm (full-height sheet)
// and lg (centered overlay) the user needs an explicit way to close.
const isAnchored = ref(false);
// The listbox owns its <input>, so we can't bind a `value` prop to seed it.
// `show(anchor, initialSearch)` stashes the seed here; onPopoverOpen drives it
// into the input once the field exists (see onPopoverOpen).
const pendingSearch = ref('');

// Source of truth: @nldd/design-system's `assets/styles/breakpoints.ts`
// (smMax: 640px, mdMin: 641px, mdMax: 1007px, lgMin: 1008px). Keep in
// sync until the design system exports breakpoints publicly - currently
// the file isn't listed in the package's `exports` field, so a deep
// import isn't supported. The previous values used `696` for mdMin
// which didn't match the design-system at all (probably a typo); aligned
// with mdMin=641 so the popover's anchored-vs-centered transition fires
// at the same width as bar-split-view's md/lg switch.
const BREAKPOINT_MD_MIN = 641;
const BREAKPOINT_LG_MIN = 1008;

function displayName(law: CorpusLaw) {
  // Prefer the API's resolved `display_name`: laws can have a dynamic
  // `name: "#output_ref"` in YAML that the backend resolves via the
  // matching action output. Without this check we'd render the raw
  // `#output_ref` string for those laws.
  if (law.display_name) return law.display_name;
  if (law.name) return law.name;
  return law.law_id.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

// Results of the server-side corpus search (`?q=`). The corpus index can
// hold thousands of laws, so the popover queries the backend rather than
// filtering a preloaded client-side list - that's the only way the search
// can reach every law, not just the first page. Ordered private-repo-first
// by the backend; `sortedLaws` keeps that order as a flat option list.
const serverLaws = ref<CorpusLaw[]>([]);
// True while a corpus query is in flight, so the UI shows a spinner instead
// of briefly flashing "no results" (which would wrongly trigger the external
// fallback) before the response lands.
const searching = ref(false);
// True when the last corpus query failed (HTTP error or network error). Kept
// distinct from "0 results" so a backend failure surfaces an error instead of
// silently masquerading as "no match" and cascading to the external fallback.
const searchFailed = ref(false);

/**
 * Flat option list for the listbox, ordered by source priority (lower =
 * higher priority, so the traject's own writable repo at priority 0 sorts
 * above the seeded central corpus). Array.sort is stable, so laws keep their
 * backend relevance order within a source. The grouping the old popover did
 * with per-source headers is now carried per row: each option shows its
 * `source_name` as supporting-text (see the template). The external
 * wetten.overheid.nl fallback only kicks in when the corpus has no match.
 */
const sortedLaws = computed(() =>
  [...serverLaws.value].sort(
    (a, b) =>
      (a.source_priority ?? Number.MAX_SAFE_INTEGER) -
      (b.source_priority ?? Number.MAX_SAFE_INTEGER),
  ),
);

// Debounce the corpus query so we don't fire a request per keystroke, and
// guard against out-of-order responses with a latest-claim.
const claimSearch = useLatest();
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

// The "only enriched" filter (shared with the Corpus juris overview) applies
// straight away: with a term it narrows the matches, without one it lists the
// enriched laws, since there are few and that is what someone ticking it wants.
watch(enrichedOnly, () => runQuery(search.value));
watch(search, runQuery);

function runQuery(q: string) {
  clearBwb();
  searchFailed.value = false;
  if (debounceTimer) clearTimeout(debounceTimer);
  const term = q.trim();
  if (enrichedOnly.value && term.length === 0) {
    searching.value = true;
    debounceTimer = setTimeout(() => runCorpusSearch(''), 0);
    return;
  }
  if (term.length < MIN_QUERY_LENGTH) {
    // Claim (and discard) a generation so any fetch already in flight
    // (debounce fired before this clear) is discarded when it resolves -
    // otherwise it would repopulate serverLaws for the cleared term and
    // fire a spurious BWB search.
    claimSearch();
    serverLaws.value = [];
    searching.value = false;
    return;
  }
  searching.value = true;
  debounceTimer = setTimeout(() => runCorpusSearch(term), 200);
}

async function runCorpusSearch(term: string) {
  const isCurrent = claimSearch();
  try {
    const query = [term ? `q=${encodeURIComponent(term)}` : '', 'limit=60', enrichedOnly.value ? 'enriched=true' : '']
      .filter(Boolean)
      .join('&');
    const url = lawsListUrl(activeTrajectRef.value, query);
    const laws = await apiFetchJson<CorpusLaw[]>(url);
    if (!isCurrent()) return; // a newer query superseded this one
    serverLaws.value = laws;
    searchFailed.value = false;
    // No match anywhere in the corpus → offer the external wetten.overheid.nl
    // search (unless the user must log in first to reach it).
    // Not while filtering on enriched laws: a law missing from that list is
    // usually in the corpus, just without a machine version.
    if (laws.length === 0 && !needsLogin.value && !enrichedOnly.value) searchBwb(term);
  } catch {
    // Backend error (500/503/…) or network failure. Surface it rather than
    // letting the empty result cascade to the external wetten.overheid.nl
    // fallback, which would mask the failure with unrelated results.
    if (isCurrent()) {
      serverLaws.value = [];
      searchFailed.value = true;
    }
  } finally {
    if (isCurrent()) searching.value = false;
  }
}

function onEnrichedChange(event: Event) {
  enrichedOnly.value = !!(event as CustomEvent<{ checked?: boolean } | undefined>).detail?.checked;
}

function bwbItemClick(result: BwbSearchResult) {
  if (needsLogin.value) {
    login();
    return;
  }
  const status = harvestStatus.value[result.bwb_id];
  if (status === 'loading') return;
  const slug = harvestSlugs.value[result.bwb_id];
  if (isAvailable(status) && slug) {
    emit('harvest-available', slug);
    close();
  } else if (!status || isTerminal(status)) {
    requestHarvest(result.bwb_id);
  }
}

function close() {
  // `?.` op hide zelf: in tests (happy-dom) is het custom element niet
  // geüpgraded en bestaat de methode niet.
  popoverRef.value?.hide?.();
}

/**
 * The listbox emits its own `input` ({ detail: { value } }) - it does NOT
 * filter internally, so we drive the corpus query straight off it. The watch
 * on `search` debounces and runs the backend query.
 */
function onListInput(e: Event) {
  search.value = (e as ValueEvent).detail?.value ?? '';
}

/**
 * Escape: the listbox owns it. With a non-empty query it clears the term and
 * calls preventDefault; once the field is empty it does nothing and lets the
 * keydown bubble up here, so the second Escape closes the popover. Reading
 * `defaultPrevented` (rather than the now-cleared value) avoids racing the
 * synchronous `input` event the clear path emits. Mirrors how Safari/Spotlight
 * handle Esc in search inputs.
 */
function onListKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && !e.defaultPrevented) close();
}

function onPopoverClose() {
  search.value = '';
  clearBwb();
  // Emit the deferred selection now: the popover has closed and already run its
  // _returnFocus, so the parent's focus-the-law wins (see selectLaw).
  if (pendingSelectLawId !== null) {
    const lawId = pendingSelectLawId;
    pendingSelectLawId = null;
    emit('select-law', lawId);
  }
}

// The chosen law id, emitted on 'select-law' only once the popover has fully
// closed (see onPopoverClose) - not here. The native popover restores focus to
// its trigger (_returnFocus) inside its async close task, just BEFORE it
// dispatches 'close'. By emitting after that, the parent's "focus the opened
// law" (scheduled on its own nextTick) runs last and wins - so focus lands on
// the law instead of snapping back to the search trigger.
let pendingSelectLawId: string | null = null;

function selectLaw(lawId: string) {
  pendingSelectLawId = lawId;
  close();
}

/**
 * Pick the presentation for the current viewport.
 *
 * On lg the popover is centered top (large dropdown overlay style). On md it
 * anchors below the trigger button (Floating UI placement), matching the
 * smaller toolbar's button-as-trigger feel. On sm it renders as a full-height
 * sheet (CSS-driven).
 *
 * Re-run on resize while open, not only on open: crossing a breakpoint with
 * the popover up otherwise leaves it in the previous viewport's layout — an
 * lg-centered panel floating loose at md, or an md-anchored panel still
 * pointing at a button that has since become the wide search field. Closing on
 * resize would be simpler but throws away what the user was typing.
 */
const lgQuery = window.matchMedia(`(min-width: ${BREAKPOINT_LG_MIN}px)`);
const mdQuery = window.matchMedia(
  `(min-width: ${BREAKPOINT_MD_MIN}px) and (max-width: ${BREAKPOINT_LG_MIN - 1}px)`,
);

function applyBreakpointLayout() {
  useCenteredPosition.value = lgQuery.matches;
  isAnchored.value = mdQuery.matches;
}

/**
 * The trigger that is on screen right now.
 *
 * Each viewport shows its own trigger (the wide search field on lg, the
 * "Zoeken" button on md, the icon on sm) and hides the others. The anchor
 * captured when the popover opened therefore goes zero-sized after a
 * breakpoint change, and Floating UI parks the panel in the top-left corner.
 * The app shell marks all three with data-search-trigger so we can re-anchor
 * without knowing the shell's layout.
 */
function visibleTrigger() {
  return (
    [...document.querySelectorAll('[data-search-trigger]')].find((el) => {
      const rect = el.getBoundingClientRect();
      return rect.width > 0 && rect.height > 0;
    }) ?? null
  );
}

/**
 * Keep an open popover in step with the viewport.
 *
 * Bound to both the media queries and `resize`: the media queries are the
 * precise signal, `resize` also catches an anchor that goes zero-sized without
 * a breakpoint being crossed. Both paths are cheap — this returns immediately
 * while closed, and while nothing that matters has moved.
 */
async function syncToViewport() {
  if (!popoverRef.value?.matches?.(':popover-open')) return;
  const wasCentered = useCenteredPosition.value;
  const wasAnchored = isAnchored.value;
  applyBreakpointLayout();
  const anchor = popoverRef.value.anchorElement;
  const anchorHidden = !anchor || anchor.getBoundingClientRect().width === 0;
  const layoutChanged =
    useCenteredPosition.value !== wasCentered || isAnchored.value !== wasAnchored;
  if (!layoutChanged && !anchorHidden) return;
  const trigger = visibleTrigger();
  if (trigger) popoverRef.value.anchorElement = trigger;
  // Let the changed centered/top/width props reach the element before asking
  // it to recompute — same ordering reason as in show().
  await nextTick();
  popoverRef.value.reposition?.();
}

// Snapshot of the open state taken before the browser's light-dismiss runs;
// see show() for why the click handler cannot read it itself.
let wasOpenOnPointerdown = false;
function onDocumentPointerdown() {
  wasOpenOnPointerdown = popoverRef.value?.matches?.(':popover-open') ?? false;
}
onMounted(() => {
  document.addEventListener('pointerdown', onDocumentPointerdown, true);
  lgQuery.addEventListener('change', syncToViewport);
  mdQuery.addEventListener('change', syncToViewport);
  window.addEventListener('resize', syncToViewport);
});
onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onDocumentPointerdown, true);
  lgQuery.removeEventListener('change', syncToViewport);
  mdQuery.removeEventListener('change', syncToViewport);
  window.removeEventListener('resize', syncToViewport);
});

/**
 * Public API: open the popover anchored to the given trigger element.
 * Parent calls this from its trigger click/focus handlers and passes
 * `event.currentTarget` (or any DOM element) as the anchor. We set
 * `anchorElement` programmatically so the same popover can be triggered
 * from multiple places (e.g. desktop search-field and mobile icon-button)
 * without each one needing a stable ID.
 *
 * Optional `initialSearch` lets the trigger seed the search value - used
 * for the Spotlight-style "type-to-open" UX where pressing a key on the
 * bar's search-field intercepts the keystroke and forwards it as the
 * initial query in the popover. Stashed in `pendingSearch` and applied by
 * onPopoverOpen (the listbox's input only exists once it's rendered).
 */
async function show(anchorEl?: EventTarget | null, initialSearch = '', event?: Event) {
  if (!popoverRef.value) return;
  // A second click on the trigger closes again. The triggers live in the app
  // shell and this popover in the routed view, so the button is not the
  // popover's invoker and the browser gives us no toggle for free.
  //
  // Reading the open state here would be too late for a pointer click: the
  // popover light-dismisses on pointerdown, so by the time this runs it is
  // already closed and re-opening looks like nothing happened. Hence the
  // pointerdown snapshot, mirroring the design system's PopupAnchorController.
  // A keyboard activation has no preceding pointerdown (detail === 0), so it
  // reads the live state instead of a stale snapshot.
  //
  // Only for a plain re-trigger: type-to-open passes a character and should
  // keep the popover open and seed it.
  // A click or key event (UIEvent) carries `detail`: the click count, 0 for a keyboard activation.
  const pointerDriven = ((event as UIEvent | undefined)?.detail ?? 0) > 0;
  const wasOpen = pointerDriven
    ? wasOpenOnPointerdown
    : (popoverRef.value.matches?.(':popover-open') ?? false);
  if (!initialSearch && wasOpen) {
    close();
    return;
  }
  // Callers pass the trigger element (the event's currentTarget).
  if (anchorEl) popoverRef.value.anchorElement = anchorEl as Element;
  pendingSearch.value = initialSearch;
  applyBreakpointLayout();
  // Wait for Vue to propagate the new `centered` / `top` / `width` props
  // onto the popover element before opening - otherwise the first
  // reposition() inside the popover's toggle handler runs against the
  // previous breakpoint's props (e.g. centered=true held over from lg)
  // and the popover lands in the wrong position. The Lit-side update on
  // prop change DOES re-trigger reposition, but in some cases the
  // initial Floating-UI run wins for the visual frame.
  await nextTick();
  popoverRef.value.show();
}

/**
 * On open: seed (or reset) the listbox's own <input>, then focus it.
 *
 * The listbox owns its input and exposes no `value` prop, so we drive the
 * seed through a real `input` event: the list reads the input, updates its
 * internal `_searchValue`, and re-emits its `{ detail: { value } }` input -
 * which onListInput turns into the corpus query. An empty `pendingSearch`
 * clears any stale value left from a previous open (the popover keeps the
 * list mounted between opens). Setting `.value` directly would be overwritten
 * by the list's own `.value` binding on the next render, hence the event.
 *
 * Listening to the popover's `open` event (rather than awaiting nextTick after
 * show()) is the only reliable focus hook: popover's own _manageFocus() runs
 * after computePosition + updateComplete and would otherwise steal focus back
 * to the host. The `open` event fires AFTER _manageFocus, so our focus wins.
 */
function onPopoverOpen() {
  const list = popoverRef.value?.querySelector('nldd-list');
  const input = list?.shadowRoot?.querySelector<HTMLInputElement>('.list__search-field-input');
  if (input) {
    input.value = pendingSearch.value;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.focus();
  }
  pendingSearch.value = '';
}

defineExpose({ show });
</script>

<template>
  <!--
    Two positioning modes - toggled by `show()` based on viewport:
    - lg+: free-positioned (centered horizontally, top-aligned). Big
      Spotlight-style overlay regardless of which trigger fired show().
    - md: anchored below the trigger button via Floating UI (default).
    On sm the bottom-sheet @media rule wins; sm-full-height fills.
  -->
  <nldd-popover
    ref="popoverRef"
    :accessible-label="SEARCH_ACCESSIBLE_LABEL"
    :width="useCenteredPosition ? '720px' : '360px'"
    placement="bottom-end"
    :centered="useCenteredPosition || null"
    :top="useCenteredPosition ? '0' : null"
    sm-full-height
    @open="onPopoverOpen"
    @close="onPopoverClose"
  >
    <nldd-container padding="16">
      <!--
        The listbox IS the search UI: it renders its own search field
        (combobox pattern), the close button sits inline in `search-bar-end`,
        and the results are its options. It does NOT filter the options itself
        - onListInput drives the server-side corpus query and we slot exactly
        the matched options back in. `height` pins the search field and scrolls
        the options below it.
      -->
      <nldd-list
        type="listbox"
        appearance="simple"
        height="min(70vh, 560px)"
        :accessible-label="SEARCH_ACCESSIBLE_LABEL"
        :translations="listTranslations"
        @input="onListInput"
        @keydown="onListKeydown"
      >
        <!-- Tijdelijk, tot de DS-fix is uitgebracht: sinds 0.8.84 haalt een
             listbox zonder rijen zijn eigen zoekveld weg ("er valt niets te
             doorzoeken"), terwijl onze rijen juist de treffers van de
             serverquery zijn. Deze verborgen rij houdt de lijst in de
             "niets gevonden"-staat, waarin het zoekveld en de Sluit-knop
             blijven staan. Weghalen zodra de listbox zijn zoekveld zelf houdt. -->
        <nldd-list-item hidden></nldd-list-item>

        <nldd-checkbox-field
          slot="toolbar"
          data-testid="search-enriched-filter"
          label="Alleen verrijkte regelwerken"
          :checked="enrichedOnly || undefined"
          @change="onEnrichedChange"
        ></nldd-checkbox-field>

        <!-- Op md anchort de popover naast de trigger - naast de popover
             klikken sluit 'm. Op sm (full-height sheet) en lg (centered
             overlay) heeft de gebruiker een expliciete sluit-knop nodig. -->
        <nldd-button
          v-if="!isAnchored"
          slot="search-bar-end"
          size="md"
          text="Sluit"
          @click="close"
        ></nldd-button>

        <!-- Interne corpus-treffers: platte lijst, eigen repo eerst (op
             bron-prioriteit), met de bron als ondertitel per rij. Rijen
             navigeren alleen naar de wet; promoten naar het traject kan
             uitsluitend via de "Regelwerk toevoegen"-flow (AddLawSheet). -->
        <nldd-list-item
          v-for="law in sortedLaws"
          :key="law.law_id"
          :data-law-id="law.law_id"
          size="md"
          button
          @click="selectLaw(law.law_id)"
        >
          <nldd-text-cell
            :text="displayName(law)"
            :supporting-text="law.source_name || undefined"
          ></nldd-text-cell>
        </nldd-list-item>

        <!-- Externe wetten.overheid.nl-treffers (alleen wanneer de corpus niets
             vond): één optie per resultaat met z'n harvest-status. -->
        <nldd-list-item
          v-for="result in bwbResults"
          :key="result.bwb_id"
          size="md"
          button
          :disabled="harvestStatus[result.bwb_id] === 'loading'
            || isPolling(harvestStatus[result.bwb_id])
            || undefined"
          @click="bwbItemClick(result)"
        >
          <nldd-text-cell
            :text="result.title"
            :supporting-text="statusText(result.bwb_id, `${result.type} - ${result.bwb_id}`)"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-icon-cell size="20">
            <nldd-icon :icon="statusIcon(result.bwb_id)"></nldd-icon>
          </nldd-icon-cell>
        </nldd-list-item>

        <!-- States zonder zichtbare opties. De listbox toont deze slot alleen
             als er een zoekterm is en geen opties zichtbaar zijn, dus de
             volgorde hieronder dekt: zoeken / mislukt / inloggen / extern
             laden / te kort / geen resultaten. -->
        <div slot="empty">
          <nldd-inline-dialog
            v-if="searching"
            text="Zoeken in de regelwerken…"
          ></nldd-inline-dialog>
          <nldd-inline-dialog
            v-else-if="searchFailed"
            variant="alert"
            text="Zoeken is mislukt"
            supporting-text="De regelwerken konden niet worden doorzocht. Probeer het opnieuw."
          ></nldd-inline-dialog>
          <nldd-inline-dialog
            v-else-if="needsLogin && !enrichedOnly && searchTerm.length >= MIN_QUERY_LENGTH"
            icon="login"
            text="Log in om externe bronnen te doorzoeken"
            supporting-text="Inloggen is vereist om wetten op te halen van wetten.overheid.nl"
          >
            <nldd-button slot="actions" appearance="primary" text="Inloggen" @click="login()"></nldd-button>
          </nldd-inline-dialog>
          <nldd-inline-dialog
            v-else-if="bwbLoading"
            text="Zoeken op wetten.overheid.nl..."
          ></nldd-inline-dialog>
          <nldd-inline-dialog
            v-else-if="searchTerm.length > 0 && searchTerm.length < MIN_QUERY_LENGTH"
            text="Typ minimaal drie tekens om te zoeken"
          ></nldd-inline-dialog>
          <nldd-inline-dialog
            v-else
            text="Geen resultaten gevonden"
            supporting-text="Pas je zoektermen of voorkeuren aan"
          ></nldd-inline-dialog>
        </div>
      </nldd-list>
    </nldd-container>
  </nldd-popover>
</template>
