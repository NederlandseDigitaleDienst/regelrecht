<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { parseFeature, dispatch, quotedValue, bareValue, traceRoot, ExecutionContext, type ParsedFeature } from '@regelrecht/frontend-shared/gherkin';
import type { NLDDNavigationSplitView } from '@nldd/design-system/navigation-split-view';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { matchStep, featureKeywords, scenarioTitle } from '../data/gherkinNl';
import { serviceInfo, type ScenarioEntry } from '../data/loadCorpus';
import { loadFailureFor, loadFailures, prepareScenarioEngine, type DemoEngine, type TraceNode } from '../engine/useDemoEngine';
import { useDemo } from '../store/demoStore';
import { useLocalePath } from '../i18n/useLocalePath';
import { useI18n } from '../i18n/index.js';
import TraceView from '../components/TraceView.vue';
import ScenarioSteps from '../components/ScenarioSteps.vue';
import { splitRunResults, type StepResult } from '../data/scenarioSteps';

// Naar een ander tabblad op naam, niet op pad: onder `/en/` leidt een
// letterlijk Nederlands pad de bezoeker ongemerkt het Nederlandse tabblad in.
const { goTo, localePath } = useLocalePath();
const { t } = useI18n();

// The scenario runner: every law's acceptance scenarios (Gherkin, canonical
// grammar) in the sidebar; the chosen feature rendered for a Dutch audience;
// "Uitvoeren" runs a scenario in the browser against the same WASM engine the
// portal uses (a second instance, with the same laws) and shows the engine's
// full trace. This is the live proof that the law behaves as the scenario says.

const route = useRoute();
const router = useRouter();
const { corpus, profile, profileKey } = useDemo();

/** One scenario's run, as the card and the trace sheet read it. */
interface ScenarioRun {
  status: 'running' | 'pass' | 'fail';
  steps: StepResult[];
  trace: TraceNode | null;
  traceText: string;
  outputs: Record<string, unknown> | null;
  error: string | null;
  wip: boolean;
}

/**
 * Whatever a step or the engine threw: an `Error`, an object `{error, trace}`,
 * or a string. Reading a property of a string gives undefined, so the
 * fallbacks below behave the same for all three.
 */
type Thrown = { error?: unknown; message?: unknown; trace?: unknown } | null | undefined;

const features = computed(() => corpus.value?.scenarios ?? []);
// The list of test files is a sheet, closed until asked for.
const splitView = ref<NLDDNavigationSplitView | null>(null);
const selectedPath = ref<string | null>(null);
const parsed = ref<ParsedFeature | null>(null);
const rawText = ref('');
const loadError = ref<unknown>(null);
const runs = reactive<Record<string, ScenarioRun>>({}); // scenario index -> { status, steps: [{status, error}], trace, outputs }
// Scenarios start collapsed: a feature with a dozen scenarios is otherwise a
// wall of steps. Each card opens on its own; a failed run opens itself so the
// failing step is in view.
const open = reactive<Record<string, boolean>>({}); // scenario index -> boolean
const runningAll = ref(false);
const anyRunning = computed(() => runningAll.value || Object.values(runs).some((r) => r.status === 'running'));
const query = ref('');

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  // Zoeken en sorteren op wat er staat, niet op de Nederlandse bron: wie in het
  // Engels "refusal" typt hoort te vinden wat er "Refusal" heet.
  const list = q ? features.value.filter((f) => `${scenarioTitle(f.title)} ${f.title} ${f.law_path}`.toLowerCase().includes(q)) : features.value;
  return [...list].sort((a, b) => scenarioTitle(a.title).localeCompare(scenarioTitle(b.title)));
});

function lawFor(feature: Pick<ScenarioEntry, 'law_path'>) {
  return corpus.value?.laws.find((l) => l.law_path === feature.law_path) ?? null;
}

async function select(path: string, { replaceRoute = false }: { replaceRoute?: boolean } = {}) {
  if (!path) return;
  selectedPath.value = path;
  splitView.value?.hidePrimarySidebarSheet?.();
  Object.keys(runs).forEach((k) => delete runs[k]);
  Object.keys(open).forEach((k) => delete open[k]);
  loadError.value = null;
  try {
    const res = await fetch(path);
    if (!res.ok) throw new Error(`Kon ${path} niet laden (${res.status})`);
    rawText.value = await res.text();
    parsed.value = parseFeature(rawText.value);
  } catch (e) {
    loadError.value = e;
    parsed.value = null;
  }
  // Het pad van het tabblad zelf (`/scenarios` of `/en/scenarios`) plus de
  // bestandsnaam ongecodeerd erachter. Niet via `resolve` met een param: deze
  // route vangt de rest van het pad op (`:featurePath(.*)`), en `resolve`
  // percent-codeert de schuine strepen daarin tot `%2F`, waarna de view zijn
  // eigen bestand niet meer terugvindt.
  const target = `${localePath('scenarios')}/${path.replace(/^\/data\/laws\//, '')}`;
  if (route.fullPath !== target) (replaceRoute ? router.replace : router.push).call(router, target);
}

watch(
  // Op `meta.page` en niet op `route.name`: zie WettenView. Onder
  // /en/scenarios heet de route `scenarios:en`, en op de naam vergelijken laat
  // dit tabblad leeg achter.
  () => [route.meta?.page, route.params.featurePath, corpus.value, profileKey.value],
  ([page, featurePath, , key], old) => {
    // Een ander persona begint bij de scenario's van zijn eigen wet, ook als
    // dit tabblad (via <keep-alive>) nog op die van het vorige persona stond.
    // Zie WettenView.
    const switched = old?.[3] != null && old[3] !== key;
    if (switched) selectedPath.value = null;
    if (page !== 'scenarios' || !corpus.value) return;
    if (featurePath && !(switched && featurePath === old[1])) {
      const path = `/data/laws/${featurePath}`;
      if (path !== selectedPath.value) select(path, { replaceRoute: true });
    } else if (!selectedPath.value) {
      const wanted = `/data/laws/${profile.value?.default_feature}`;
      const first = features.value.find((f) => f.path === wanted) ?? features.value[0];
      if (first) select(first.path, { replaceRoute: true });
    }
  },
  { immediate: true },
);

const selectedFeature = computed(() => features.value.find((f) => f.path === selectedPath.value) ?? null);
const selectedLaw = computed(() => (selectedFeature.value ? lawFor(selectedFeature.value) : null));
// The engine refused the law these scenarios are about (a type-check finding,
// RFC-037): every run would fail with "law not found", which hides the cause.
const selectedLoadFailure = computed(() => (selectedLaw.value ? loadFailureFor(selectedLaw.value.id) : null));

const showText = ref(false);

/** Scenario steps with the background prepended. */
function stepsOf(scenario: ParsedFeature['scenarios'][number]) {
  return [...(parsed.value?.background ?? []), ...scenario.steps];
}

/**
 * Run one scenario on the scenario engine. Data steps register scoped sources
 * there; the sources are cleared before every run so scenarios never see each
 * other's rows. The portal's engine and its persona data are never touched.
 */
async function run(index: number) {
  const scenario = parsed.value?.scenarios[index];
  if (!scenario || !corpus.value || runs[index]?.status === 'running') return;
  const wip = scenario.tags.includes('@wip');
  // Reactive on purpose: the run mutates this object step by step, and the card
  // has to follow. A plain object assigned into `runs` would only be tracked
  // through `runs[index]`, not through this reference.
  const state = reactive<ScenarioRun>({ status: 'running', steps: [], trace: null, traceText: '', outputs: null, error: null, wip });
  runs[index] = state;
  // Let the card repaint (status icon, loading button) before the engine work.
  await new Promise((resolve) => setTimeout(resolve));
  const ctx = new ExecutionContext();
  const t0 = performance.now();
  const timings: string[] = [];
  const lap = (label: string, from: number) => timings.push(`${label} ${(performance.now() - from).toFixed(1)}ms`);
  let startedAt = performance.now();
  let e: DemoEngine;
  try {
    e = await prepareScenarioEngine(corpus.value);
  } catch (err) {
    state.error = `Engine niet beschikbaar: ${(err as Thrown)?.message ?? err}`;
    state.status = 'fail';
    open[index] = true;
    return;
  }
  lap('engine', startedAt);
  const refused = selectedLoadFailure.value;
  if (refused) {
    state.error = t('scenario.law_refused', { id: refused.id, path: refused.path, message: refused.message });
    state.status = 'fail';
    open[index] = true;
    return;
  }
  startedAt = performance.now();
  e.clearDataSources();
  lap('clearDataSources', startedAt);
  try {
    for (const step of stepsOf(scenario)) {
      startedAt = performance.now();
      const match = matchStep(step.text);
      const record: StepResult = { status: 'pending', error: null };
      state.steps.push(record);
      if (!match) {
        record.status = 'fail';
        record.error = t('scenario.error.unknown_step');
        break;
      }
      const { entry, args } = match;
      const typed = args.map((raw, i) => (entry.argTypes[i] === 'number' ? bareValue(raw) : quotedValue(raw)));
      const table = step.dataTable ?? null;
      try {
        if (entry.action === 'evaluate_outputs') {
          const outputs = String(typed[0]).split(',').map((s) => s.trim());
          // Quoted captures are typed by value_typing (inferred), so a law id
          // or output name that reads as a number arrives as one; the engine
          // takes text, as the shared dispatcher's `text()` hands it over.
          evaluateWithTrace(ctx, e, String(typed[1]), outputs, state);
        } else if (entry.action === 'evaluate') {
          evaluateWithTrace(ctx, e, String(typed[1]), [String(typed[0])], state);
        } else {
          await dispatch(ctx, e, entry.action, [...typed, ...entry.literals], table, { loadDependency: async () => {} });
        }
        record.status = 'pass';
        lap(`step:${entry.action}`, startedAt);
      } catch (err) {
        record.status = 'fail';
        record.error = String((err as Thrown)?.message ?? (err as Thrown)?.error ?? err);
        break;
      }
    }
    const failed = state.steps.some((s) => s.status === 'fail');
    state.status = failed ? 'fail' : 'pass';
    // A failure opens the card so the failing step is in view; a pass leaves
    // the card as the presenter had it.
    if (failed) open[index] = true;
  } finally {
    e.clearDataSources();
    console.debug(`[scenario-timing] ${scenario.name}: total ${(performance.now() - t0).toFixed(1)}ms | ${timings.join(' | ')}`);
  }
}

/** The 1-based number of the failing step within the scenario's own steps, or 0 when it is a background step. */
function failedStepNumber(index: number) {
  const r = runs[index];
  const at = r?.steps?.findIndex((s) => s.status === 'fail') ?? -1;
  if (at < 0) return null;
  const bg = parsed.value?.background?.length ?? 0;
  return at < bg ? 0 : at - bg + 1;
}

function resultTag(index: number) {
  const r = runs[index];
  if (r?.status === 'pass') return { color: 'success', text: t('scenario.passed') };
  if (r?.status === 'fail') {
    const n = failedStepNumber(index);
    const text = n === 0 ? t('scenario.failed.background') : n ? t('scenario.failed.at_step', { n }) : t('scenario.failed');
    // A @wip scenario is known not to pass yet (the Rust runner skips it); its
    // failure is expected, not a regression.
    return r.wip ? { color: 'warning', text: `${text} (@wip)` } : { color: 'critical', text };
  }
  return null;
}

function statusIcon(index: number) {
  const s = runs[index]?.status;
  return s === 'pass' ? 'check-mark-circle' : s === 'fail' ? 'dismiss-circle' : s === 'running' ? 'clock' : 'circle-dashed';
}

// An `nldd-icon` colour, not an icon-cell one: the status icon sits inside the
// row's disclosure segment, and that segment turns every icon *cell* in it a
// quarter as the scenario opens. Only the chevron is meant to turn.
function statusColor(index: number) {
  const r = runs[index];
  return r?.status === 'pass' ? 'success' : r?.status === 'fail' ? (r.wip ? 'warning' : 'critical') : 'secondary-content';
}

function evaluateWithTrace(ctx: ExecutionContext, e: DemoEngine, lawId: string, outputs: string[], state: ScenarioRun) {
  if (!ctx.calculationDate) throw new Error(t('scenario.error.no_reference_date'));
  try {
    const result = e.executeMultipleWithTrace(lawId, outputs, ctx.parameters, ctx.calculationDate);
    ctx.result = result;
    ctx.executed = true;
    ctx.error = null;
    state.trace = traceRoot<TraceNode>(result.trace);
    state.traceText = result.trace_text ?? '';
    state.outputs = result.outputs ?? {};
  } catch (err) {
    ctx.error = err;
    ctx.executed = true;
    ctx.result = null;
    const thrown = err as Thrown;
    state.traceText = thrown?.trace ? renderTraceFallback(thrown) : '';
    state.error = String(thrown?.error ?? thrown?.message ?? err);
    // Assertions after a failed run report the failure themselves.
  }
}

function renderTraceFallback(err: { error?: unknown; message?: unknown }) {
  return `Uitvoering mislukt: ${err.error ?? err.message ?? err}`;
}


/**
 * One scenario run by hand opens its trace straight away: that is what the
 * presenter clicks through next. "Run all" does not, it would open one sheet
 * after the other.
 */
async function runAndShow(index: number) {
  await run(index);
  if (runs[index]?.traceText) activeTrace.value = index;
}

async function runAll() {
  if (runningAll.value) return;
  runningAll.value = true;
  try {
    for (let i = 0; i < (parsed.value?.scenarios.length ?? 0); i += 1) {
      // eslint-disable-next-line no-await-in-loop
      await run(i);
    }
  } finally {
    runningAll.value = false;
  }
}

const summary = computed(() => {
  const all = Object.values(runs);
  return { total: parsed.value?.scenarios.length ?? 0, pass: all.filter((r) => r.status === 'pass').length, fail: all.filter((r) => r.status === 'fail').length };
});

const activeTrace = ref<number | null>(null); // scenario index whose trace is shown
const traceScenario = computed(() => (activeTrace.value === null ? null : runs[activeTrace.value] ?? null));
const traceSheet = ref<NLDDSheet | null>(null);
watch(activeTrace, async (index) => {
  if (index === null) return traceSheet.value?.hide?.();
  await nextTick();
  traceSheet.value?.show?.();
});

/** A scenario's run results, split at the background (see splitRunResults). */
function runResults(index: number) {
  return splitRunResults(runs[index]?.steps, parsed.value?.background?.length ?? 0);
}

/** The run status of the scenario's own steps, without the background's. */
function scenarioResults(index: number) {
  return runResults(index).scenario;
}

/** The error of a failed background step in this scenario's run, if any. */
function backgroundError(index: number) {
  return runResults(index).backgroundError;
}

const fileName = computed(() => selectedPath.value?.split('/').pop() ?? '');
</script>

<template>
  <nldd-navigation-split-view ref="splitView" primary-sidebar-as-sheet :primary-sidebar-accessible-label="t('app.tabs.scenarios')">
    <nldd-split-view-pane slot="sidebar" has-content background="tinted">
      <nldd-page sticky-header background="inherit">
        <nldd-container slot="header" padding="12" gap="8">
          <nldd-top-title-bar :text="t('app.tabs.scenarios')" :supporting-text="t.plural(features.length, 'scenario.files')"></nldd-top-title-bar>
          <nldd-search-field :placeholder="t('scenario.search')" size="sm" :value="query" @input="query = $event.detail?.value ?? $event.target.value"></nldd-search-field>
        </nldd-container>
        <nldd-container padding-inline="8" padding-bottom="16">
          <nldd-list type="navigation" :accessible-label="t('scenario.files.label')">
            <!-- A law found for `f` means the corpus is loaded, and `lawFor(f)` is checked just before. -->
            <nldd-list-item v-for="f in filtered" :key="f.path" size="sm" button :selected="f.path === selectedPath || undefined" @click="select(f.path)">
              <nldd-text-cell size="sm" :text="scenarioTitle(f.title)" :supporting-text="lawFor(f) ? serviceInfo(corpus!, lawFor(f)!.service).name : f.law_path"></nldd-text-cell>
            </nldd-list-item>
            <nldd-inline-dialog slot="empty" :text="t('scenario.none_found')" :supporting-text="t('scenario.none_found.hint')"></nldd-inline-dialog>
          </nldd-list>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" has-content>
      <nldd-page landmarks="page" sticky-header>
        <nldd-container slot="header" padding="8">
          <nldd-toolbar size="sm">
            <nldd-toolbar-item slot="start">
              <nldd-button size="sm" appearance="neutral-tinted" start-icon="checklist" :text="t('app.tabs.scenarios')" :supporting-text="`${features.length}`" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-title v-if="parsed" slot="start" :text="scenarioTitle(parsed.feature)" :supporting-text="fileName"></nldd-toolbar-title>
            <nldd-toolbar-item slot="end" v-if="parsed">
              <nldd-segmented-control size="sm" width="fit-content" :value="showText ? 'text' : 'steps'" @change="showText = $event.detail?.value === 'text'">
                <nldd-segmented-control-item value="steps" :text="t('app.tabs.scenarios')"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="text" :text="t('scenario.view.file')"></nldd-segmented-control-item>
              </nldd-segmented-control>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end" v-if="parsed">
              <nldd-button size="sm" appearance="primary" start-icon="play" :text="t('scenario.run_all')" :loading="runningAll || undefined" :disabled="(anyRunning && !runningAll) || undefined" @click="runAll"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end" v-if="parsed && selectedLaw">
              <nldd-button size="sm" appearance="neutral-tinted" start-icon="book" :text="t('wet.tile.action.law_text')" @click="goTo('wetten', { lawId: selectedLaw.id })"></nldd-button>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>

        <nldd-simple-section v-if="loadFailures.length" width="full">
          <nldd-banner
            variant="critical"
            :text="selectedLoadFailure ? t('scenario.law_not_loaded', { id: selectedLoadFailure.id }) : t.plural(new Set(loadFailures.map((f) => f.id)).size, 'scenario.laws_not_loaded')"
            :supporting-text="t('scenario.engine_refused', { details: loadFailures.map((f) => `${f.id} (${f.path}): ${f.message}`).join('; ') })"
          ></nldd-banner>
        </nldd-simple-section>
        <nldd-simple-section v-if="loadError" width="full">
          <nldd-banner variant="critical" :text="t('scenario.load_failed')" :supporting-text="String(loadError)"></nldd-banner>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="!parsed" height="60vh">
          <nldd-inline-dialog icon="checklist" :text="t('scenario.pick')" :supporting-text="t('scenario.pick.hint')">
            <nldd-button slot="actions" appearance="primary" size="sm" :text="t('app.tabs.scenarios')" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
          </nldd-inline-dialog>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="showText" width="full">
          <nldd-code-viewer language="gherkin" wrap>{{ rawText }}</nldd-code-viewer>
        </nldd-simple-section>
        <nldd-simple-section v-else width="full">
          <nldd-container gap="16">
          <nldd-box>
            <nldd-container padding="12" layout="row" gap="12" vertical-alignment="center">
              <nldd-icon-cell icon="checklist" color="secondary"></nldd-icon-cell>
              <nldd-title-cell size="5" :text="scenarioTitle(parsed.feature)" :overline="featureKeywords().Feature" :supporting-text="parsed.background?.length ? `${featureKeywords().Background}: ${t.plural(parsed.background.length, 'scenario.steps')}` : undefined"></nldd-title-cell>
              <!-- The run summary sits with the feature, next to its title, as the result
                   tag does on each scenario card. In the toolbar it stood between 32px
                   controls; no tag or badge size reaches that height. -->
              <nldd-tag v-if="summary.pass + summary.fail > 0" :color="summary.fail ? 'critical' : 'success'" :text="summary.fail ? `${t('scenario.summary.pass', { n: summary.pass })}, ${t('scenario.summary.fail', { n: summary.fail })}` : t('scenario.summary.pass', { n: summary.pass })"></nldd-tag>
            </nldd-container>
            <!-- The background is the shared premise of every scenario below, so it
                 stays in view; only the scenario cards collapse. -->
            <nldd-container v-if="parsed.background?.length" padding-inline="16" padding-bottom="12" gap="8">
              <nldd-text-cell color="accent" :text="`**${featureKeywords().Background}:**`"></nldd-text-cell>
              <ScenarioSteps :steps="parsed.background" />
            </nldd-container>
          </nldd-box>
          <nldd-card v-for="(scenario, index) in parsed.scenarios" :key="index" :accessible-label="scenarioTitle(scenario.name)">
            <!-- The whole header opens the scenario, except its buttons. The row is
                 cut in two: a disclosure segment (status, title, result, chevron)
                 that toggles and carries aria-expanded, and plain cells holding the
                 buttons. A row that is a button itself cannot hold "Uitvoeren":
                 a button inside a button is invalid and a screen reader would
                 announce only the outer one. -->
            <nldd-container slot="header" padding-inline="12" padding-block="4">
              <nldd-list dividers="never">
                <nldd-list-item :expanded="!!open[index]">
                  <nldd-list-item-segment button disclosure width="full" @click="open[index] = !open[index]">
                    <nldd-icon-cell icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
                    <nldd-spacer-cell size="8"></nldd-spacer-cell>
                    <!-- nldd-icon in a cell, not an nldd-icon-cell: the segment turns
                         every icon cell in it, and only the chevron should turn. -->
                    <nldd-cell><nldd-icon :icon="statusIcon(index)" :color="statusColor(index)" size="24"></nldd-icon></nldd-cell>
                    <nldd-spacer-cell size="12"></nldd-spacer-cell>
                    <nldd-title-cell size="5" :text="scenarioTitle(scenario.name)" :supporting-text="scenario.tags.join(' ') || undefined"></nldd-title-cell>
                    <!-- One height across the action row: an md tag and xs buttons are both
                         24px; no tag size matches an sm button. -->
                    <!-- `?.` only for the type checker: the v-if guarantees the tag. -->
                    <nldd-cell v-if="resultTag(index)"><nldd-tag :color="resultTag(index)?.color" :text="resultTag(index)?.text"></nldd-tag></nldd-cell>
                  </nldd-list-item-segment>
                  <!-- Spacer cells, as the other list rows in the demo space their
                       cells: the segment stops at the result tag, and without
                       them the tag and both buttons touch. -->
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-cell>
                    <nldd-button size="xs" appearance="secondary" start-icon="play" :text="t('scenario.run')" :loading="runs[index]?.status === 'running' || undefined" :disabled="(anyRunning && runs[index]?.status !== 'running') || undefined" @click="runAndShow(index)"></nldd-button>
                  </nldd-cell>
                  <nldd-spacer-cell v-if="runs[index]?.traceText" size="8"></nldd-spacer-cell>
                  <nldd-cell v-if="runs[index]?.traceText">
                    <nldd-button size="xs" appearance="neutral-tinted" start-icon="list" :text="t('scenario.trace')" @click="activeTrace = index"></nldd-button>
                  </nldd-cell>
                </nldd-list-item>
              </nldd-list>
            </nldd-container>
            <nldd-container v-if="open[index]" padding-inline="16" padding-bottom="16" gap="12">
              <nldd-banner v-if="runs[index]?.error && !runs[index]?.steps?.length" variant="critical" :text="t('scenario.run_failed')" :supporting-text="runs[index].error"></nldd-banner>
              <nldd-banner v-if="backgroundError(index)" variant="critical" :text="t('scenario.failed.background')" :supporting-text="backgroundError(index)"></nldd-banner>
              <ScenarioSteps :steps="scenario.steps" :results="scenarioResults(index)" />
            </nldd-container>
          </nldd-card>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <!-- The engine's trace is wide; a 320px inspector column cuts every line,
         so it opens in a broad sheet, as the tile's "Berekening" does. As wide
         as a hall screen allows: the sheet clamps it to the viewport. -->
    <Teleport to="body">
      <nldd-sheet ref="traceSheet" placement="right" width="1400px" :accessible-label="t('scenario.trace.label')" @close="activeTrace = null">
        <!-- A trace scenario means `activeTrace` is set. -->
        <nldd-page v-if="traceScenario">
          <nldd-container slot="header" padding="12">
            <nldd-top-title-bar :text="t('scenario.trace.label')" :supporting-text="parsed?.scenarios[activeTrace!]?.name" :dismiss-text="t('scenario.close')" @dismiss="activeTrace = null"></nldd-top-title-bar>
          </nldd-container>
          <nldd-container padding="16" gap="16">
            <nldd-banner v-if="traceScenario.error" variant="critical" :text="t('scenario.run_failed')" :supporting-text="traceScenario.error"></nldd-banner>
            <nldd-list v-if="traceScenario.outputs" appearance="box-tinted" :accessible-label="t('scenario.outputs')">
              <nldd-list-item v-for="(v, k) in traceScenario.outputs" :key="k" size="sm">
                <nldd-text-cell size="sm" :text="String(k)"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="JSON.stringify(v)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <TraceView :text="traceScenario.traceText" />
          </nldd-container>
        </nldd-page>
      </nldd-sheet>
    </Teleport>
  </nldd-navigation-split-view>
</template>
