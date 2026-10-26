<script setup lang="ts">
import { computed, nextTick, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue';
import { VueFlow, useVueFlow, type NodeDragEvent, type NodeMouseEvent, type ViewportTransform } from '@vue-flow/core';
import type { NLDDNavigationSplitView } from '@nldd/design-system/navigation-split-view';
import { centerOf, graphView } from '../walkthrough/graphBridge';
import { Background } from '@vue-flow/background';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import GraphLawNode from '../components/graph/GraphLawNode.vue';
import GraphBoxNode from '../components/graph/GraphBoxNode.vue';
import GraphItemNode from '../components/graph/GraphItemNode.vue';
import LawGroupTree from '../components/LawGroupTree.vue';
import { buildGraph, lawShape, neighbourhood } from '../graph/lawGraph';
import { fieldSpec, formatValue } from '../data/format';
import { lineageFromTrace, type LineageNode } from '../data/lineage';
import { serviceInfo, type LawEntry } from '../data/loadCorpus';
import { useDemo } from '../store/demoStore';
import { useLocalePath } from '../i18n/useLocalePath';
import { useI18n } from '../i18n/index.js';

// Naar een ander tabblad op naam, niet op pad: onder `/en/` leidt een
// letterlijk Nederlands pad de bezoeker ongemerkt het Nederlandse tabblad in.
const { goTo } = useLocalePath();
const { t } = useI18n();

// The dependency graph as the POC drew it: every selected law and its direct
// neighbours as a box with its register sources, its inputs from other laws
// and its outputs (the whole corpus is laid out, so switching to "Alles" adds
// laws around the ones already in view without moving them), an edge
// from each input to the output that supplies it, and on every item the value
// the engine found for the active persona. The profile chooses the laws that
// tell its story (`graph_laws`); the sidebar lets the presenter add or drop
// laws. The canvas is vue-flow, as in the editor: the design system has no
// graph component (documented exception).

const demo = useDemo();
const { corpus, profile, portalLaws, dataVersion } = demo;
// Same store id as the <VueFlow> below, otherwise fitView talks to a different instance.
const { fitView, zoomIn, zoomOut, setCenter, getViewport, dimensions, findNode } = useVueFlow({ id: 'demo-graph' });

// The recorded walkthrough follows and steers the graph's camera through this
// bridge (walkthrough/graphBridge.ts): panning and zooming leave no clicked
// element behind for it to replay.
onMounted(() => {
  graphView.api = { setCenter, getViewport, dimensions, moveNode };
});
onUnmounted(() => {
  if (graphView.api?.setCenter === setCenter) graphView.api = null;
});
function onViewportChange(viewport: ViewportTransform) {
  graphView.onView?.(centerOf(viewport, dimensions.value));
}
/** Where the layout puts a node; a drag is recorded and replayed relative to it. */
function laidOut(id: string) {
  return graph.value.nodes.find((n) => n.id === id)?.position ?? null;
}
let dragStarted = 0;
function onNodeDragStart() {
  dragStarted = performance.now();
}
function onNodeDragStop({ nodes, node }: NodeDragEvent) {
  const ms = Math.round(performance.now() - dragStarted);
  for (const n of nodes?.length ? nodes : [node]) {
    const base = laidOut(n.id);
    if (base) graphView.onNodeMove?.({ id: n.id, dx: Math.round(n.position.x - base.x), dy: Math.round(n.position.y - base.y), ms });
  }
}
/** Put a node `dx, dy` away from its place in the layout, gliding over `duration` ms. */
function moveNode(id: string, dx: number, dy: number, duration = 0) {
  const node = findNode(id);
  const base = laidOut(id);
  if (!node || !base) return false;
  const to = { x: base.x + dx, y: base.y + dy };
  const from = { ...node.position };
  const steps = Math.max(1, Math.round(duration / 16));
  let i = 0;
  const step = () => {
    i += 1;
    const f = i / steps;
    node.position = { x: from.x + (to.x - from.x) * f, y: from.y + (to.y - from.y) * f };
    if (i < steps) setTimeout(step, 16);
  };
  step();
  return true;
}
// The law list is a sheet, closed until asked for; the presets live in the toolbar.
const splitView = ref<NLDDNavigationSplitView | null>(null);

const allLaws = computed(() => (corpus.value ? [...corpus.value.latestById.values()].sort((a, b) => a.name.localeCompare(b.name)) : []));
const groups = computed(() => {
  const byService = new Map<string, LawEntry[]>();
  for (const law of allLaws.value) {
    const service = law.service;
    if (!byService.has(service)) byService.set(service, []);
    byService.get(service)!.push(law); // set on the line above
  }
  // `allLaws` is empty without a corpus, so there is one here.
  return [...byService.entries()].sort((a, b) => a[0].localeCompare(b[0])).map(([service, laws]) => ({ service, info: serviceInfo(corpus.value!, service), laws }));
});

// Only called with a corpus loaded (see applyPreset).
function referencesOf(law: LawEntry) {
  return lawShape(law).inputs.map((i) => i.ref.regulation).filter((id) => corpus.value!.latestById.has(id));
}
/** A set of laws plus everything they read from, transitively. */
function withDependencies(laws: LawEntry[]) {
  const wanted = new Set<string>();
  const queue = [...laws];
  while (queue.length) {
    const law = queue.shift()!; // the queue is not empty
    if (wanted.has(law.id)) continue;
    wanted.add(law.id);
    // `referencesOf` keeps only ids the corpus holds.
    for (const ref of referencesOf(law)) queue.push(corpus.value!.lawById(ref)!);
  }
  return wanted;
}

// ---- selection ----------------------------------------------------------------
const focus = ref<string | null>(null);
// Staat de focus er omdat het profiel hem koos (`graph_focus`), dan zoomt de
// graaf op die wet in plaats van op het hele verhaal:
// Claudia's verhaal telt zeventien wetten, en uitgezoomd daarop is de
// precariobelasting niet meer te lezen. Een klik van de presentator neemt het
// over.
const focusFromProfile = ref(false);

function focusProfileLaw() {
  const id = profile.value?.graph_focus;
  const law = id && corpus.value?.latestById.has(id) ? id : null;
  focus.value = law;
  focusFromProfile.value = !!law;
}

const selected = ref(new Set<string>());
const preset = ref('verhaal'); // 'verhaal' | 'portaal' | 'alles' | '' (hand-picked)

function applyPreset(name: string) {
  if (!corpus.value || !profile.value) return;
  // Bij een profielwissel zet focusProfileLaw hem direct daarna weer aan.
  focusFromProfile.value = false;
  preset.value = name;
  if (name === 'alles') {
    selected.value = new Set(allLaws.value.map((l) => l.id));
  } else if (name === 'portaal') {
    selected.value = withDependencies(portalLaws.value);
  } else {
    const story = (profile.value.graph_laws ?? []).filter((id) => corpus.value!.latestById.has(id)); // checked at the top
    if (story.length) selected.value = new Set(story);
    else {
      const d = profile.value.default_law;
      const law = d ? corpus.value!.lawByPath(d.law_path, d.service) : null;
      selected.value = law ? withDependencies([law]) : new Set<string>();
    }
  }
}
// Een wissel terwijl een ander tabblad open staat (<keep-alive>) past de graaf
// aan terwijl hij onzichtbaar is; fitView meet dan niets. Bij terugkomst dus
// nog één keer passend maken, maar alleen dan: wie op de graaf zelf wisselt en
// daarna rondkijkt, wil zijn beeld terug als hij even weg is geweest.
let active = false;
let refitOnActivate = false;
// Op de profielsleutel en niet op het profielobject: een taalwissel levert
// een nieuw corpus en dus een nieuw profielobject, en dan hoort de keuze van
// de presentator (wetten, focus, zoom) te blijven staan.
watch([() => demo.profileKey.value, () => !!corpus.value], () => {
  applyPreset('verhaal');
  focusProfileLaw();
  if (!active) refitOnActivate = true;
}, { immediate: true });
onActivated(() => {
  active = true;
  if (!refitOnActivate) return;
  refitOnActivate = false;
  refit();
});
onDeactivated(() => {
  active = false;
});

// Wie zelf wetten kiest, neemt het beeld over van de profielfocus: anders zoomt
// elke herberekening van de graaf terug naar die ene wet (zie refit).
function toggle(lawId: string) {
  focusFromProfile.value = false;
  const next = new Set(selected.value);
  if (next.has(lawId)) next.delete(lawId);
  else next.add(lawId);
  selected.value = next;
  preset.value = '';
}
function only(lawId: string) {
  focusFromProfile.value = false;
  selected.value = new Set([lawId]);
  preset.value = '';
}
// As in the POC: the selected laws and everything directly connected to them.
const shownIds = computed(() => neighbourhood(selected.value, allLaws.value));
const shownLaws = computed(() => allLaws.value.filter((l) => shownIds.value.has(l.id)));

// ---- the persona's values ----------------------------------------------------
// Every shown law is evaluated for the active persona; the outputs go on the
// output items, the register values the trace resolved on the source items,
// and an input shows the value of the output it reads.
const values = computed(() => {
  void dataVersion.value;
  const outputs: Record<string, Record<string, string>> = {};
  const sources: Record<string, Record<string, string>> = {};
  const refs = new Map<string, unknown>(); // `${law}#${output}` -> raw value, from cross-law references in traces
  // Only reached for shown laws, and there are none without a corpus.
  const fmt = (lawId: string, name: string, v: unknown) => formatValue(v, fieldSpec(corpus.value!.lawById(lawId)?.doc, name));
  for (const law of shownLaws.value) {
    let evaluation;
    try {
      evaluation = demo.evaluate(law);
    } catch {
      continue;
    }
    if (!evaluation?.ok) continue;
    outputs[law.id] = Object.fromEntries(Object.entries(evaluation.outputs ?? {}).map(([k, v]) => [k, fmt(law.id, k, v)]));
    const walk = (nodes: LineageNode[]) => {
      for (const n of nodes) {
        if (n.kind === 'value') {
          sources[n.law] ??= {};
          sources[n.law]![n.name] = fmt(n.law, n.name, n.value); // set on the line above
        } else {
          refs.set(`${n.law}#${n.name}`, n.value);
          walk(n.children ?? []);
        }
      }
    };
    if (evaluation.trace) walk(lineageFromTrace(evaluation.trace, law.id, demo.personaParams()));
  }
  const inputs: Record<string, Record<string, string>> = {};
  for (const law of shownLaws.value) {
    for (const input of lawShape(law).inputs) {
      const key = `${input.ref.regulation}#${input.ref.output}`;
      // Leest de wet dit uit een besloten zaak (caseRefs), dan telt wat zij
      // daar las: een losse run van de andere wet kent de aanvraag niet.
      const viaCase = (law.caseRefs ?? []).some((r) => r.name === input.name) ? sources[law.id]?.[input.name] : undefined;
      // A reference without an output indexes and formats as 'undefined', as before.
      const output = input.ref.output as string;
      const fromOutputs = outputs[input.ref.regulation]?.[output];
      const v = viaCase ?? fromOutputs ?? (refs.has(key) ? fmt(input.ref.regulation, output, refs.get(key)) : undefined);
      if (v !== undefined) {
        inputs[law.id] ??= {};
        inputs[law.id]![input.name] = v; // set on the line above
      }
    }
  }
  return { outputs, sources, inputs };
});

// ---- the graph ------------------------------------------------------------------
// Every law is laid out so the picture never shifts when "Alles" comes on;
// only the neighbourhood of the selection is visible.
const graph = computed(() => buildGraph(allLaws.value, values.value, focus.value, shownIds.value));

function onNodeClick({ node }: NodeMouseEvent) {
  focusFromProfile.value = false;
  if (node.type === 'law') focus.value = focus.value === node.id ? null : node.id;
  else if (node.type === 'item' && node.data.ref && selected.value.has(node.data.ref.regulation)) focus.value = node.data.ref.regulation;
}
function onNodeDoubleClick({ node }: NodeMouseEvent) {
  if (node.type === 'law') goTo('wetten', { lawId: node.id });
}
function onPaneClick() {
  focus.value = null;
  focusFromProfile.value = false;
}
/**
 * Everything shown, or only the profile's focus law. Not the focus law plus its
 * neighbours: the layout runs left to right along the chain, so a law and its
 * neighbours can sit at opposite ends, and their bounds are nearly the whole
 * graph again.
 */
function refit() {
  const onFocus = focusFromProfile.value && focus.value && shownIds.value.has(focus.value);
  // `onFocus` implies a focus law.
  const options = onFocus ? { padding: 0.2, maxZoom: 1, nodes: [focus.value!] } : { padding: 0.1, nodes: [...shownIds.value] };
  setTimeout(() => fitView(options), 50);
}
/**
 * "Passend maken" in the toolbar and "Alles in beeld" by the zoom buttons:
 * always the whole picture, also after a profile put one law in focus.
 */
function fitAll() {
  focusFromProfile.value = false;
  refit();
}
// vue-flow's fit-view-on-init runs before the nodes exist (the corpus arrives
// async); refit whenever the set of laws changes, and when the profile puts a
// law in focus. A law the viewer clicks is selected where it is: zooming out
// to the whole picture made exactly that law unreadable.
watch(
  () => [shownLaws.value.map((l) => l.id).join(), !!focus.value],
  async ([ids], old) => {
    await nextTick();
    if (old && ids === old[0] && !focusFromProfile.value) return;
    refit();
  },
);

const focusLaw = computed(() => (focus.value ? corpus.value?.lawById(focus.value) : null));
// The graph's edges connect laws of the corpus, so the corpus is loaded and holds both ends.
const uses = computed(() => (focus.value ? graph.value.edges.filter((e) => e.data.from === focus.value).map((e) => corpus.value!.lawById(e.data.to)!) : []));
const usedBy = computed(() => (focus.value ? graph.value.edges.filter((e) => e.data.to === focus.value).map((e) => corpus.value!.lawById(e.data.from)!) : []));
function unique(laws: LawEntry[]) {
  return [...new Map(laws.map((l) => [l.id, l])).values()];
}
</script>

<template>
  <nldd-navigation-split-view ref="splitView" primary-sidebar-as-sheet :primary-sidebar-accessible-label="t('wet.graph.sidebar.label')" :inspector-accessible-label="t('wet.graph.inspector.label')">
    <nldd-split-view-pane slot="sidebar" has-content background="tinted">
      <nldd-page sticky-header background="inherit">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="t('wet.graph.sidebar.label')" :supporting-text="t('wet.graph.selection', { chosen: selected.size, shown: shownLaws.length })"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding-inline="8" padding-bottom="16">
          <LawGroupTree :groups="groups" mode="check" :checked="selected" :supporting-text="(law) => (!selected.has(law.id) && shownIds.has(law.id) ? t('wet.graph.neighbour') : undefined)" @toggle="toggle" />
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" has-content>
      <nldd-page landmarks="page" sticky-header>
        <nldd-container slot="header" padding="8">
          <nldd-toolbar size="sm">
            <nldd-toolbar-item slot="start">
              <nldd-button size="sm" appearance="neutral-tinted" start-icon="books" :text="t('wet.sidebar.label')" :supporting-text="`${selected.size}`" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-title slot="start" :text="t('wet.graph.title')" :supporting-text="t('wet.graph.subtitle', { laws: shownLaws.length, edges: graph.edges.filter((e) => !e.hidden).length, persona: profile?.name ?? t('wet.graph.persona.fallback') })" max-width="480px"></nldd-toolbar-title>
            <nldd-toolbar-item slot="end">
              <nldd-segmented-control size="sm" width="fit-content" :value="preset" @change="applyPreset($event.detail?.value)">
                <nldd-segmented-control-item value="verhaal" :text="t('wet.graph.preset.story')"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="portaal" :text="profile?.name ?? t('wet.graph.preset.portal')"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="alles" :text="t('wet.graph.preset.all')"></nldd-segmented-control-item>
              </nldd-segmented-control>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button size="sm" appearance="neutral-tinted" start-icon="binoculars" :text="t('wet.graph.fit')" @click="fitAll()"></nldd-button>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
        <nldd-simple-section v-if="!shownLaws.length" height="60vh">
          <nldd-inline-dialog icon="centralized-network" :text="t('wet.graph.empty.title')" :supporting-text="t('wet.graph.empty.body')">
            <nldd-button slot="actions" appearance="primary" size="sm" :text="t('wet.sidebar.label')" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
          </nldd-inline-dialog>
        </nldd-simple-section>
        <div v-else class="graph-canvas">
          <VueFlow
            id="demo-graph"
            :nodes="graph.nodes"
            :edges="graph.edges"
            :min-zoom="0.1"
            :max-zoom="2"
            fit-view-on-init
            :nodes-connectable="false"
            :elevate-edges-on-select="false"
            @node-click="onNodeClick"
            @node-double-click="onNodeDoubleClick"
            @node-drag-start="onNodeDragStart"
            @node-drag-stop="onNodeDragStop"
            @pane-click="onPaneClick"
            @nodes-initialized="refit"
            @viewport-change="onViewportChange"
          >
            <template #node-law="{ data }"><GraphLawNode :data="data" /></template>
            <template #node-box="{ data }"><GraphBoxNode :data="data" /></template>
            <template #node-item="{ data }"><GraphItemNode :data="data" /></template>
            <Background />
          </VueFlow>
          <!-- De zoomknoppen van vue-flow vervangen door knoppen uit het design
               system: die bracht zijn eigen stylesheet en eigen vormgeving mee,
               die naast de rest van de app stond en niets van het thema wist.
               `useVueFlow` levert dezelfde acties, dus alleen de knoppen zijn
               anders. -->
          <!-- Liggend, niet staand zoals vue-flow ze zette: nldd-button-bar is
               een horizontale groep en kent geen staande variant. Zelf rechtop
               zetten zou tegen het component in werken; dit is dezelfde bediening
               in de vorm die het design system ervoor heeft. -->
          <!-- `neutral-tinted` is de standaardvariant van de balk, en de enige
               die hier een zichtbare hover geeft. `neutral-base` zette de balk
               op wit met een hover van 0.974: dat verschil zie je niet, en
               omdat de knop geen rand heeft viel hij onder de muis weg in
               plaats van op te lichten. `neutral-transparent` is nog erger, die
               heeft helemaal geen hover-vulling. Getint geeft 0.923 in rust en
               0.898 onder de muis. -->
          <nldd-button-bar class="graph-zoom" appearance="neutral-tinted">
            <nldd-icon-button icon="add" :text="t('wet.graph.zoom_in')" @click="zoomIn()"></nldd-icon-button>
            <nldd-icon-button icon="remove" :text="t('wet.graph.zoom_out')" @click="zoomOut()"></nldd-icon-button>
            <nldd-button-bar-divider></nldd-button-bar-divider>
            <!-- `fitAll()`, not a bare `fitView()`: that one also fits the
                 hidden laws and zoomed out further than needed. Does the same
                 as "Passend maken" in the toolbar; that one is there for whoever
                 reads the bar, this one for whoever is working in the canvas
                 with a hand on the zoom buttons. -->
            <nldd-icon-button icon="fit-to-view" :text="t('wet.graph.fit_all')" @click="fitAll()"></nldd-icon-button>
          </nldd-button-bar>
        </div>
      </nldd-page>
    </nldd-split-view-pane>

    <!-- A focus law means the corpus is loaded. -->
    <nldd-split-view-pane v-if="focusLaw" slot="inspector" has-content background="tinted">
      <nldd-page background="inherit">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="focusLaw.name" :supporting-text="serviceInfo(corpus!, focusLaw.service).name" :dismiss-text="t('wet.graph.close')" @dismiss="focus = null"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="12" gap="12">
          <nldd-button-group orientation="horizontal" size="sm">
            <nldd-button appearance="secondary" size="sm" start-icon="book" :text="t('wet.graph.open_in_laws')" @click="goTo('wetten', { lawId: focusLaw.id })"></nldd-button>
            <nldd-button appearance="neutral-tinted" size="sm" :text="t('wet.graph.only_this')" @click="only(focusLaw.id)"></nldd-button>
          </nldd-button-group>
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('wet.graph.reads_from') }}</nldd-text></nldd-container>
            <nldd-list appearance="box-base" :accessible-label="t('wet.graph.reads_from')">
              <nldd-list-item v-if="uses.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('wet.graph.no_law_in_view')"></nldd-text-cell></nldd-list-item>
              <nldd-list-item v-for="l in unique(uses)" :key="l.id" size="sm" button @click="focus = l.id">
                <nldd-text-cell size="sm" :text="l.name" :supporting-text="serviceInfo(corpus!, l.service).name"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('wet.graph.read_by') }}</nldd-text></nldd-container>
            <nldd-list appearance="box-base" :accessible-label="t('wet.graph.read_by')">
              <nldd-list-item v-if="usedBy.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('wet.graph.no_law_in_view')"></nldd-text-cell></nldd-list-item>
              <nldd-list-item v-for="l in unique(usedBy)" :key="l.id" size="sm" button @click="focus = l.id">
                <nldd-text-cell size="sm" :text="l.name" :supporting-text="serviceInfo(corpus!, l.service).name"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>
