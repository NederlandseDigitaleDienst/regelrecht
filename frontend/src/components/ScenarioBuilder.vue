<script lang="ts">
import type { ExecutionData } from './ScenarioForm.vue';
import type { Expectation } from '../utils/outputFormat';

// Parsed-version cache, MODULE-scoped (shared across all ScenarioBuilder
// instances) so it survives remounts - switching the panel view away and back,
// or opening another article, remounts this component. The data-source column
// type map (`rebuildExternalFieldTypeMap`) is derived from these YAMLs, while
// the WASM engine that gates whether `loadAllDependencies` re-fetches a dep is
// itself a shared singleton that stays warm across mounts. A per-instance cache
// would therefore be empty on every remount once the engine is warm, forcing a
// full dependency re-fetch each time. Invalidated per-traject in
// `fetchLawVersions`. Assumes at most one ScenarioBuilder is mounted at a time
// (the scenario panel): `versionsCacheTrajectRef` is shared, so two concurrent
// instances with different trajectRefs would invalidate each other's cache.
const versionsCache: Record<string, string[]> = {};
let versionsCacheTrajectRef: string | null = null;

/** Which sheet the parent opens for a scenario's result. */
type DetailsView = 'trace' | 'graph';

/** What `executed` hands the parent: a scenario's last run and how to re-run it. */
export interface ScenarioExecutedPayload {
  result: ExecutionData['result'];
  traceText: unknown;
  error: unknown;
  running: boolean;
  expectations: Record<string, Expectation>;
  scenarioName: string;
  outputName: string | null;
  index: number;
  reload: () => void;
  view: DetailsView;
}
</script>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onBeforeUnmount } from 'vue';
import { useDependencies } from '../composables/useDependencies';
import { loadLawVersions } from '../composables/useEngine';
import { useScenarios, isScenarioMismatch } from '../composables/useScenarios';
import { lawVersionsUrl } from '../composables/corpusUrls';
import { apiFetchJson } from '../lib/apiFetch';
import { useLatest } from '../lib/useLatest';
import { parseFeature } from '../gherkin/parser';
import { mapFeatureToForm, getEffectiveSetup, formStateToGherkin, syncEditedValues } from '../gherkin/formMapper';
import { matchStatus, humanize, formatOutputValue, normalizeForCompare, expectationsFromAssertions } from '../utils/outputFormat';
import * as yaml from 'js-yaml';
import { buildArticleMap, buildTypeMap, buildExternalFieldTypeMap } from '../utils/articleMapping';
import ScenarioForm from './ScenarioForm.vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import type { FormState } from '../gherkin/formMapper';
import type { EditorEngine } from '../composables/useEngine';
import type { FieldMeta } from '../utils/articleMapping';

/** An article as the article/type maps read it (the maps' own parameter type). */
type MappedArticle = NonNullable<Parameters<typeof buildArticleMap>[0]>[number];

/** A law document as buildExternalFieldTypeMap reads it. */
type MappedLawDoc = { articles?: readonly MappedArticle[] | null };

type ScenarioFormInstance = InstanceType<typeof ScenarioForm>;

const props = withDefaults(defineProps<{
  lawId: string;
  lawYaml?: string | null;
  engine?: EditorEngine | null;
  ready?: boolean;
  /** The shared article/law load is still in flight. The pane renders us during
   *  that load (no `lawYaml` yet) so our indicator can span BOTH phases as one
   *  mounted element: article -> scenario files -> dependency laws. Two separate
   *  indicators handing off would each restart the DS anti-flash timer on mount,
   *  giving spinner -> 1s blank -> spinner. */
  articleLoading?: boolean;
  /** Articles array from useLaw() for article mapping */
  articles?: MappedArticle[];
  /** Active traject ref. Required for scenario writes; reads route
   *  through the matching traject's backend so a save is visible
   *  without a corpus reload. */
  // Single-instance assumption: the module-scoped versionsCache keys on this
  // trajectRef (see the <script> block at the top), so at most one
  // ScenarioBuilder may be mounted at a time.
  trajectRef?: string | null;
}>(), {
  lawYaml: null,
  engine: null,
  ready: false,
  articleLoading: false,
  articles: () => [],
  trajectRef: null,
});

const emit = defineEmits<{
  executed: [payload: ScenarioExecutedPayload];
  'dirty-change': [dirty: boolean];
}>();

// --- Article mapping ---
const articleMap = computed(() => buildArticleMap(props.articles));
// Datatype mapping: drives the per-type scenario input controls (boolean ->
// switch, amount -> currency field, etc.) in ScenarioForm.
const typeMap = computed(() => buildTypeMap(props.articles));
// External data-source column types, collected from the current law + the
// already-fetched dependency YAMLs. Rebuilt when the dependency load settles.
const externalFieldTypeMap = ref(new Map<string, FieldMeta>());

// --- Dependencies ---
const {
  loading: depsLoading,
  progress: depsProgress,
  error: depsError,
  loadAllDependencies,
  loadImplementors,
} = useDependencies();

// --- Scenario loading ---
const lawIdRef = computed(() => props.lawId);
const trajectRefRef = computed(() => props.trajectRef);
const {
  scenarios: scenarioFiles,
  selectedScenario: selectedScenarioFile,
  featureText,
  loading: scenariosLoading,
  saving,
  error: scenariosError,
  saveError,
  selectScenario: selectScenarioFile,
  saveScenario,
} = useScenarios(lawIdRef, trajectRefRef);

// Mismatch warning: the selected scenario file declares execution targets
// that do not include the opened law. Running it would evaluate that other
// law - surface this instead of letting the run fail confusingly.
const selectedScenarioEntry = computed(
  () => scenarioFiles.value.find((sf) => sf.filename === selectedScenarioFile.value) || null,
);
const selectedScenarioMismatchTargets = computed(() =>
  selectedScenarioEntry.value && isScenarioMismatch(selectedScenarioEntry.value, props.lawId)
    ? selectedScenarioEntry.value.target_law_ids
    : null,
);
const mismatchSupportingText = computed(() =>
  selectedScenarioMismatchTargets.value
    ? `Dit scenario evalueert '${selectedScenarioMismatchTargets.value.join("', '")}', niet dit regelwerk ('${props.lawId}'). Uitvoeren gebruikt dat andere regelwerk.`
    : '',
);

const formState = ref<FormState | null>(null);
const saveSuccess = ref(false);
const isDirty = ref(false);
const selectedScenarioIndex = ref<number | null>(null);
const scenarioSheetEl = ref<NLDDSheet | null>(null);

// Name of the data source or collection parameter the active ScenarioForm
// is drilled into (null = scenario overview). Reported by ScenarioForm via
// @drill-change; the top-title-bar back button uses it to pop one level
// back out.
const drilledSourceName = ref<string | null>(null);

watch(selectedScenarioIndex, async (idx) => {
  // Opening / switching a scenario always lands on its overview, so reset
  // every form's drill state (only the active form can be drilled into).
  drilledSourceName.value = null;
  await nextTick();
  scenarioRefs.value.forEach((f) => f?.clearDrill?.());
  if (idx !== null) {
    // Baseline the editable state so Save can disappear again once the
    // user manually reverts every change (markDirty re-compares).
    dirtyBaseline = editSnapshot();
    isDirty.value = false;
    scenarioSheetEl.value?.show();
  } else {
    scenarioSheetEl.value?.hide();
  }
});

// Serialised editable surface: the (in-place edited) scenario objects plus
// each ScenarioForm's local input values. Compared against the baseline so
// reverting an edit clears the dirty state.
let dirtyBaseline = '';
function editSnapshot() {
  try {
    return JSON.stringify({
      s: formState.value?.scenarios ?? null,
      f: scenarioRefs.value.map((r) => (r?.getFormValues ? r.getFormValues() : null)),
    });
  } catch {
    return `dirty-${Date.now()}`;
  }
}

const currentScenarioName = computed(() =>
  selectedScenarioIndex.value !== null
    ? formState.value?.scenarios?.[selectedScenarioIndex.value]?.name ?? ''
    : '',
);
function onTitleBack() {
  const idx = selectedScenarioIndex.value;
  if (idx !== null) scenarioRefs.value[idx]?.clearDrill?.();
}

watch(isDirty, (val) => emit('dirty-change', val));

function markDirty() {
  isDirty.value = editSnapshot() !== dirtyBaseline;
}

// The same map the result views compare against, rendered the way they
// render an outcome, so the card says `geen` / `onbekend` where the sheet
// does (RFC-036).
function scenarioExpectations(index: number) {
  const assertions = formState.value?.scenarios?.[index]?.assertions || [];
  return Object.entries(expectationsFromAssertions(assertions)).map(([name, expected]) => ({
    name: humanize(name),
    value: humanize(formatOutputValue(normalizeForCompare(expected), null)),
  }));
}

// Parse feature file when loaded
watch(featureText, (text) => {
  isDirty.value = false;
  selectedScenarioIndex.value = null;
  if (!text) {
    formState.value = null;
    return;
  }
  try {
    const parsed = parseFeature(text);
    formState.value = mapFeatureToForm(parsed);
  } catch {
    formState.value = null;
  }
});

// Cache for fetched law version lists, scoped to the active traject so a
// traject switch doesn't return the previous traject's bodies. The dependency
// loader needs *every* version of a law (not just today's-best) so the engine
// can pick the one in force on the scenario's calculation date.
async function fetchLawVersions(lawId: string): Promise<string[]> {
  if (versionsCacheTrajectRef !== props.trajectRef) {
    Object.keys(versionsCache).forEach((k) => delete versionsCache[k]);
    versionsCacheTrajectRef = props.trajectRef;
  }
  if (versionsCache[lawId]) return versionsCache[lawId];
  const yamls = await apiFetchJson(lawVersionsUrl(props.trajectRef, lawId), {
    errorMessage: (status) => `Failed to fetch versions of law '${lawId}': ${status}`,
  });
  // The endpoint returns every version's YAML text.
  const list: string[] = Array.isArray(yamls) ? yamls : [];
  // Only cache a non-empty result. An empty array (unknown/not-yet-harvested
  // law) must stay uncached so a retry after harvest re-fetches rather than
  // returning the stale `[]` - `[]` is truthy, so caching it would short-
  // circuit the `if (versionsCache[lawId])` guard forever.
  if (list.length > 0) versionsCache[lawId] = list;
  return list;
}

// --- Dependencies ready tracking ---
const depsReady = ref(false);

// --- Load dependencies when law YAML changes ---
const claimDependencyLoad = useLatest();

// Debounced mirror of props.lawYaml. While the user types in the text or
// machine pane, `lawYaml` changes on every keystroke (currentLawYaml re-dumps
// the whole doc), which would re-run the expensive dependency reload + corpus
// scan and toggle depsReady - making the scenario panel flicker. We only let
// the cascade below fire ~300ms after the last edit. Same setTimeout debounce
// pattern as ScenarioForm.vue's execute.
const debouncedLawYaml = ref(props.lawYaml);
let lawYamlDebounce: ReturnType<typeof setTimeout> | undefined;

watch(() => props.lawYaml, (val, prev) => {
  // First population or cleared→set (no prior law loaded): apply immediately so
  // the initial dependency load isn't delayed by 300ms. Any change from an
  // existing value - keystroke edits, but also switching to another article of
  // the already-open law - debounces.
  clearTimeout(lawYamlDebounce);
  if (!prev) {
    debouncedLawYaml.value = val;
    return;
  }
  lawYamlDebounce = setTimeout(() => {
    debouncedLawYaml.value = val;
  }, 300);
});

onBeforeUnmount(() => clearTimeout(lawYamlDebounce));

// Collect external data-source column types from the current law plus the
// dependency YAMLs already fetched into versionsCache (parsed with js-yaml).
// Called once the dependency load settles so the data-source tables can render
// typed cells. Tolerates unparseable/text-only versions.
function rebuildExternalFieldTypeMap() {
  const docs: MappedLawDoc[] = [{ articles: props.articles || [] }];
  for (const [lawId, versions] of Object.entries(versionsCache)) {
    for (const v of Array.isArray(versions) ? versions : []) {
      try {
        const doc = yaml.load(v);
        // The cached YAML is not schema-checked; the map reads it defensively.
        if (doc && typeof doc === 'object') docs.push(doc as MappedLawDoc);
      } catch (e) {
        console.warn(`Skipped an unparseable cached version of '${lawId}' while building the type map:`, e);
      }
    }
  }
  externalFieldTypeMap.value = buildExternalFieldTypeMap(docs);
}

// Run the dependency cascade for the current law + scenarios. Reads the
// latest prop/state values at call time (not captured watch args) so a
// debounced run always uses the freshest inputs.
async function runDependencyLoad() {
  const lawYaml = debouncedLawYaml.value;
  if (!lawYaml || !props.ready || !props.engine) return;
  const isCurrent = claimDependencyLoad();
  depsReady.value = false;

  const mainLawId = await loadAllDependencies(lawYaml, props.engine, fetchLawVersions);
  if (!isCurrent()) return;

  // Also load dependencies from scenario background + per-scenario steps
  if (formState.value) {
    const allDeps = new Set<string>();
    for (const dep of formState.value.background?.dependencies || []) allDeps.add(dep);
    for (const sc of formState.value.scenarios || []) {
      for (const dep of sc.setup?.dependencies || []) allDeps.add(dep);
    }
    for (const depId of allDeps) {
      try {
        // Fetch versions unconditionally to populate versionsCache for the
        // data-source type map (same rationale as loadAllDependencies - the
        // map is built from cached YAMLs, independent of engine state); load
        // into the shared engine only if it isn't already present.
        const yamls = await fetchLawVersions(depId);
        if (!props.engine.hasLaw(depId)) {
          loadLawVersions(props.engine, yamls, depId);
        }
      } catch (e) {
        console.warn(`Failed to load scenario dependency '${depId}':`, e);
      }
    }
  }

  if (isCurrent()) {
    // The explicitly-declared deps are loaded - the panel is usable now, so
    // mark ready and let scenarios auto-execute. Implementing regulations
    // (IoC) load in the background: their corpus scan can be slow and is
    // best-effort, so it must not gate the panel. `loadImplementors` is
    // guarded to run at most once per law.
    //
    // Deliberately fire-and-forget - there is no AbortController. If this
    // component unmounts mid-scan the promise keeps running, which is safe:
    // Vue ignores ref writes after unmount, the shared WASM engine outlives
    // the component, and the guard resets on error so a fresh mount retries.
    depsReady.value = true;
    rebuildExternalFieldTypeMap();
    if (mainLawId) {
      // Implementor regulations load in the background, populating versionsCache
      // after the rebuild above. Re-type once they settle so any source:{}
      // fields they declare get picked up (idempotent; skipped if superseded).
      Promise.resolve(
        loadImplementors(mainLawId, props.engine, fetchLawVersions, props.trajectRef),
      )
        .then(() => { if (isCurrent()) rebuildExternalFieldTypeMap(); })
        .catch((e) => console.warn('Implementor load / re-type failed (non-fatal):', e));
    }
  }
}

// `debouncedLawYaml`, `props.ready` and `formState` settle on separate ticks
// during the initial load. Without coalescing, each settle fires this watch
// and starts (then abandons, via the latest-guard) a full dependency scan - up
// to four overlapping corpus-wide reloads per open. A short debounce collapses
// the burst into a single run after the inputs have settled.
let depsScheduleTimer: ReturnType<typeof setTimeout> | undefined;
function scheduleDependencyLoad() {
  clearTimeout(depsScheduleTimer);
  depsScheduleTimer = setTimeout(runDependencyLoad, 30);
}
onBeforeUnmount(() => clearTimeout(depsScheduleTimer));

watch(
  [debouncedLawYaml, () => props.ready, formState],
  scheduleDependencyLoad,
  { immediate: true },
);

// --- Template refs for ScenarioForm instances ---
const scenarioRefs = ref<(ScenarioFormInstance | null)[]>([]);

// --- Per-scenario result tracking ---
const scenarioResults = ref(new Map<number, ExecutionData>());

function onScenarioResult(index: number, data: ExecutionData) {
  const updated = new Map(scenarioResults.value);
  updated.set(index, data);
  scenarioResults.value = updated;
}

function scenarioStatus(index: number) {
  const data = scenarioResults.value.get(index);
  if (!data) return null;
  if (data.error) return 'failed';
  if (!data.result) return null;

  const scenario = formState.value?.scenarios[index];
  if (!scenario) return null;

  const expectations = expectationsFromAssertions(scenario.assertions);
  const names = Object.keys(expectations);
  if (names.length === 0) return null;

  for (const name of names) {
    if (matchStatus(name, data.result.outputs?.[name], expectations) === 'failed') return 'failed';
  }
  return 'passed';
}

// Reset results and refs when formState changes (new scenario file loaded)
watch(formState, () => {
  scenarioResults.value = new Map();
  scenarioRefs.value = [];
});

// --- Auto-execute all scenarios sequentially when deps are ready ---
const claimAutoExecute = useLatest();

watch(
  [depsReady, formState],
  async ([ready, state]) => {
    if (!ready || !state || !state.scenarios?.length) return;
    const isCurrent = claimAutoExecute();

    // Wait one tick so ScenarioForm refs are mounted
    await nextTick();
    if (!isCurrent()) return;

    for (let i = 0; i < state.scenarios.length; i++) {
      if (!isCurrent()) return;
      const formRef = scenarioRefs.value[i];
      if (formRef?.execute) {
        formRef.execute();
        // Collect result after execution
        const data = formRef.getExecutionData?.();
        if (data) onScenarioResult(i, data);
      }
    }
  },
);

// --- Details handler: emit to right panel ---
// `view` indicates which sheet the parent should open: 'trace' for
// the execution-trace sheet (default), 'graph' for the law-graph sheet.
function onShowDetails(index: number, view: DetailsView = 'trace') {
  // Prefer fresh data from the form ref, but its state may have been reset
  // after a save/reload - fall back to the cached result in that case.
  const formRef = scenarioRefs.value[index];
  const fresh = formRef?.getExecutionData?.();
  const hasFresh = fresh && (fresh.result || fresh.traceText || fresh.error);
  const data = hasFresh ? fresh : scenarioResults.value.get(index);
  const scenarioName = formState.value?.scenarios[index]?.name || '';
  // Always emit so the sheet opens: the result sheet itself handles the
  // loading / error (with reload) / empty states instead of the button
  // being a dead gate.
  emit('executed', {
    result: data?.result || null,
    traceText: data?.traceText || null,
    error: data?.error || null,
    // Always false today: execute() is synchronous (see the CONTRACT
    // note on ScenarioForm.execute) so `running` is reset in its finally
    // before getExecutionData() is read here. The "Bezig met uitvoeren…"
    // branch in ExecutionTraceView and the lastRunning/lastReload
    // scaffolding in EditorApp are therefore unreachable *by design* -
    // deliberately kept so the async path lights up for free if that
    // contract is ever lifted. Not dead code to be removed in isolation.
    running: !!fresh?.running,
    expectations: data?.expectations || {},
    scenarioName,
    // Forward the scenario's entry output so the graph view can pin
    // its "▶ start" marker to the right output leaf.
    outputName: data?.outputName || null,
    index,
    // Bound to this builder + scenario so the result sheet's reload
    // action re-runs exactly the right scenario regardless of how many
    // ScenarioBuilder instances (panes) exist.
    //
    // Known limitation: `index` is captured by value and the result
    // sheet can outlive the scenario sheet. It stays correct in practice
    // because scenario count/order is stable across an inputs-only save
    // and cancelEdits() no longer replaces formState - so nothing
    // reindexes scenarios while the sheet is open, and the UI has no
    // reorder/delete-scenario affordance. If the index ever did go out
    // of bounds, reExecute()'s optional chaining makes it a safe no-op
    // (empty result) rather than running the wrong scenario.
    reload: () => reExecute(index),
    view,
  });
}

// Re-run a scenario from the result sheet's reload action, then refresh the
// sheet with the fresh outcome. ScenarioForm.execute() runs the WASM engine
// in-process and synchronously (no API call, no await; `running` is reset in
// its finally before it returns), so the result/error is already set by the
// time onShowDetails reads it back via getExecutionData().
function reExecute(index: number) {
  const formRef = scenarioRefs.value[index];
  if (formRef?.execute) formRef.execute();
  onShowDetails(index);
}

// Memoized setup per scenario (avoids new object on every render)
const scenarioSetups = computed(() => {
  const state = formState.value;
  if (!state) return [];
  return state.scenarios.map((_, i) => getEffectiveSetup(state, i));
});

function onScenarioFileSelect(event: Event) {
  const filename = (event.target as HTMLSelectElement).value;
  if (filename) selectScenarioFile(filename);
}

async function onSave() {
  if (!formState.value || !selectedScenarioFile.value) return;

  saveSuccess.value = false;
  try {
    // Sync edited input values back to formState before serializing.
    // This must be inside the try so that a throw from syncEditedValues
    // (e.g. unexpected form shape) surfaces as a save error rather than
    // failing silently.
    for (let i = 0; i < (formState.value.scenarios || []).length; i++) {
      const formRef = scenarioRefs.value[i];
      if (!formRef?.getFormValues) continue;
      const values = formRef.getFormValues();
      syncEditedValues(formState.value, i, values);
    }

    const gherkin = formStateToGherkin(formState.value);
    await saveScenario(selectedScenarioFile.value, gherkin);
    saveSuccess.value = true;
    isDirty.value = false;
    // The just-saved state is the new clean baseline. Re-snapshot here so
    // the dirty check stays correct even if the sheet is *not* closed on
    // save (today selectedScenarioIndex is nulled below, but don't let the
    // invariant depend on that): without this, reverting back to the saved
    // state would still compare unequal to the pre-edit baseline and keep
    // the Save footer wrongly visible.
    dirtyBaseline = editSnapshot();
    selectedScenarioIndex.value = null;
    setTimeout(() => { saveSuccess.value = false; }, 3000);
  } catch (e) {
    // The composable sets saveError on its own failures. For sync/serialise
    // errors that happen before saveScenario, set it manually so the user
    // still sees the banner instead of an unexplained no-op.
    // Whatever was thrown; the banner falls back to String() for a non-Error.
    if (!saveError.value) saveError.value = e as Error;
  }
}

async function onSaveAndShow() {
  const idx = selectedScenarioIndex.value;
  await onSave();
  if (saveError.value || idx === null) return;
  // Saving reloads featureText which resets each ScenarioForm's local result
  // via its setup/scenario watcher. Re-execute so the result sheet reflects
  // the saved scenario rather than a stale cache or an empty state.
  await nextTick();
  const formRef = scenarioRefs.value[idx];
  if (formRef?.execute) {
    // execute() returns the post-execution data synchronously; use the
    // return value directly so we never fall back to the pre-save cache.
    const data = formRef.execute();
    if (data) onScenarioResult(idx, data);
  }
  onShowDetails(idx);
}

function cancelEdits() {
  // Discard unsaved edits *without* replacing formState. Re-parsing it
  // remounts the whole overview - clearing cached results/refs (via the
  // formState watcher) and resetting the scenarios-pane scroll position.
  // Edits live entirely in the edited ScenarioForm's local refs (only
  // synced into formState on save), so asking that form to re-init from
  // its unchanged props discards them while leaving the overview - and
  // its scroll position - intact, exactly as when nothing was edited.
  const idx = selectedScenarioIndex.value;
  if (isDirty.value && idx !== null) {
    scenarioRefs.value[idx]?.discardEdits?.();
  }
  isDirty.value = false;
  selectedScenarioIndex.value = null;
}

defineExpose({ save: onSave });
</script>

<template>
  <!-- Overview. Wrapped in a positioned container so the loading overlay can
       cover the whole pane. -->
  <div class="sb-pane">
    <nldd-simple-section>
      <nldd-dropdown v-if="scenarioFiles.length > 1" size="md">
        <select
          :value="selectedScenarioFile"
          @change="onScenarioFileSelect"
        >
          <option v-for="sf in scenarioFiles" :key="sf.filename" :value="sf.filename">
            {{ isScenarioMismatch(sf, lawId) ? '⚠ ' + sf.filename : sf.filename }}
          </option>
        </select>
      </nldd-dropdown>

      <template v-if="selectedScenarioMismatchTargets">
        <nldd-banner
          variant="warning"
          text="Scenario hoort bij een ander regelwerk"
          :supporting-text="mismatchSupportingText"
        ></nldd-banner>
        <nldd-spacer size="16"></nldd-spacer>
      </template>

      <nldd-inline-dialog v-if="saveSuccess" text="Opgeslagen"></nldd-inline-dialog>
      <nldd-inline-dialog v-if="saveError" variant="alert" text="Opslaan mislukt" :supporting-text="saveError.message || String(saveError)"></nldd-inline-dialog>

      <nldd-inline-dialog v-if="depsError" variant="alert" text="Fout" :supporting-text="String(depsError)"></nldd-inline-dialog>

      <nldd-inline-dialog
        v-if="scenariosError"
        variant="alert"
        text="Scenario's konden niet worden geladen"
        :supporting-text="scenariosError.message || String(scenariosError)"
      ></nldd-inline-dialog>

      <template v-if="formState">
        <nldd-collection layout="grid" item-min-width="320px">
          <nldd-card v-for="(scenario, i) in formState.scenarios" :key="i">
            <nldd-container slot="header" padding-top="16" padding-inline="16">
              <nldd-title size="4"><h3>{{ scenario.name }}</h3></nldd-title>
            </nldd-container>
            <nldd-container padding="16">
              <template v-if="scenarioExpectations(i).length">
                <nldd-title size="6"><h4>Verwachte uitkomsten</h4></nldd-title>
                <nldd-spacer size="4"></nldd-spacer>
                <nldd-list appearance="simple">
                  <nldd-list-item v-for="(exp, j) in scenarioExpectations(i)" :key="j" size="sm">
                    <nldd-text-cell size="sm" :text="exp.name"></nldd-text-cell>
                    <nldd-text-cell size="sm" horizontal-alignment="right" :text="exp.value"></nldd-text-cell>
                  </nldd-list-item>
                </nldd-list>
              </template>
              <template v-else>
                <nldd-text size="sm" color="secondary">Geen verwachte uitkomsten gedefinieerd</nldd-text>
              </template>
            </nldd-container>
            <nldd-container slot="footer" padding-inline="16" padding-bottom="16">
              <!-- Not gated on a cached result: onShowDetails always emits and
                   the result sheet handles its own loading / empty / error
                   (with reload) states. Disabling here turned the buttons into
                   a dead end while dependencies were still loading (or after a
                   save reset the cached result), so the user could never open
                   the trace/graph to retry. -->
              <nldd-button-group orientation="horizontal">
                <nldd-button
                  text="Resultaat"
                  @click="onShowDetails(i, 'trace')"
                ></nldd-button>
                <nldd-button
                  appearance="secondary"
                  text="Graaf"
                  @click="onShowDetails(i, 'graph')"
                ></nldd-button>
                <nldd-button
                  text="Bewerk"
                  @click="selectedScenarioIndex = i"
                ></nldd-button>
              </nldd-button-group>
            </nldd-container>
          </nldd-card>
        </nldd-collection>
      </template>

      <nldd-inline-dialog
        v-else-if="!articleLoading && !scenariosLoading && !depsLoading && !scenariosError"
        text="Geen scenario's beschikbaar voor dit artikel."
      ></nldd-inline-dialog>
    </nldd-simple-section>
    <!-- Full-pane loading overlay, shown across the WHOLE scenario story: the
         article/law load, then the scenario files, then their dependency laws
         ("X/Y regelwerken geladen"). Deliberately one condition over all three so
         this element stays mounted throughout - the DS resets its anti-flash
         timer on every connectedCallback, so handing off between two separate
         indicators produced spinner -> 1s blank -> spinner. Staying mounted
         runs the 1000ms timer once: quick loads still never flash, and a slow
         one shows a single continuous spinner. -->
    <nldd-activity-indicator
      v-if="articleLoading || scenariosLoading || depsLoading"
      backdrop
      show-text
      :text="depsLoading ? depsProgress : 'Scenario\'s laden'"
      style="position: absolute; inset: 0;"
    ></nldd-activity-indicator>
  </div>

  <!-- Edit scenario in a side sheet. All ScenarioForm instances stay
       mounted so auto-execute can cache results for the overview cards. -->
  <Teleport to="body">
    <nldd-sheet
      v-if="formState"
      ref="scenarioSheetEl"
      placement="right"
      width="640px"
      @close="cancelEdits"
    >
      <nldd-page sticky-header :sticky-footer="isDirty || undefined">
        <!-- Overview header: scenario name (revealed in the bar on scroll
             via the content-title anchor), no back. -->
        <nldd-top-title-bar
          v-if="!drilledSourceName"
          slot="header"
          :text="currentScenarioName"
          collapse-anchor="scenario-title-anchor"
          dismiss-text="Annuleer"
          @dismiss="cancelEdits"
        ></nldd-top-title-bar>
        <!-- Drilled-in header: its own bar - a back button to the scenario
             overview (the data-source heading lives in the content). -->
        <nldd-top-title-bar
          v-else
          slot="header"
          :back-text="currentScenarioName"
          dismiss-text="Annuleer"
          @back="onTitleBack"
          @dismiss="cancelEdits"
        ></nldd-top-title-bar>
        <nldd-simple-section>
          <template v-if="!drilledSourceName">
            <nldd-title id="scenario-title-anchor" size="3"><h2>{{ currentScenarioName }}</h2></nldd-title>
            <nldd-spacer size="16"></nldd-spacer>
          </template>
          <!-- scenarioSetups maps formState.scenarios one to one, so entry i
               exists and is non-null for every scenario i. -->
          <ScenarioForm
            v-for="(scenario, i) in formState.scenarios"
            v-show="selectedScenarioIndex === i"
            :key="i"
            :ref="(el) => { scenarioRefs[i] = el as ScenarioFormInstance | null; }"
            :scenario="scenario"
            :setup="scenarioSetups[i]!"
            :engine="engine"
            :ready="ready"
            :law-id="lawId"
            :article-map="articleMap"
            :type-map="typeMap"
            :external-field-type-map="externalFieldTypeMap"
            @show-details="() => onShowDetails(i)"
            @executed="(data) => onScenarioResult(i, data)"
            @change="markDirty"
            @drill-change="(name) => { if (selectedScenarioIndex === i) drilledSourceName = name; }"
          />
        </nldd-simple-section>
        <nldd-container v-if="isDirty" slot="footer" padding="16">
          <nldd-button-group orientation="vertical">
            <nldd-button
              appearance="primary"
              size="md"
              data-testid="save-scenarios-btn"
              :disabled="saving || undefined"
              :text="saving ? 'Opslaan…' : 'Opslaan'"
              @click="onSave"
            ></nldd-button>
            <nldd-button
              size="md"
              :disabled="saving || undefined"
              text="Opslaan en toon resultaat"
              @click="onSaveAndShow"
            ></nldd-button>
          </nldd-button-group>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<style scoped>
/* Positioning context for the full-pane loading overlay. min-height fills the
   pane's scroll viewport so the backdrop covers the whole area, not just the
   (possibly empty) content. Flex column so the simple-section grows to the full
   height - its empty-state inline-dialog then self-centers like elsewhere,
   instead of sitting at the top. The absolute overlay is unaffected. */
.sb-pane {
  display: flex;
  position: relative;
  min-height: 100%;
  flex-direction: column;
}
</style>
