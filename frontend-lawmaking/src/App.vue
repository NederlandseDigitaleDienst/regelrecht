<template>
  <nldd-shell>
    <nldd-page sticky-header>
      <!-- Sticky header: branding + playback/zoom/view controls -->
      <nldd-container slot="header" padding="8">
        <nldd-toolbar size="md">
          <nldd-toolbar-title
            slot="start"
            text="Wetgevingsproces"
            supporting-text="Van wetsvoorstel tot geldend recht - het wetgevingsproces als GitFlow"
          >
            <img
              slot="media"
              src="/assets/rijkswapen.svg"
              alt="Rijkswapen"
              class="app-logo"
            />
          </nldd-toolbar-title>

          <!-- Playback: step navigation -->
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              icon="chevron-double-left"
              text="Naar begin"
              :disabled="activeStep <= 0 || undefined"
              @click="resetSteps"
            ></nldd-icon-button>
            <nldd-menu-item
              slot="overflow"
              icon="chevron-double-left"
              text="Naar begin"
              :disabled="activeStep <= 0 || undefined"
              @select="resetSteps"
            ></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              icon="chevron-left"
              text="Stap terug"
              :disabled="activeStep <= 0 || undefined"
              @click="stepBack"
            ></nldd-icon-button>
            <nldd-menu-item
              slot="overflow"
              icon="chevron-left"
              text="Stap terug"
              :disabled="activeStep <= 0 || undefined"
              @select="stepBack"
            ></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              icon="chevron-right"
              text="Stap vooruit"
              :disabled="activeStep >= maxStep || undefined"
              @click="stepForward"
            ></nldd-icon-button>
            <nldd-menu-item
              slot="overflow"
              icon="chevron-right"
              text="Stap vooruit"
              :disabled="activeStep >= maxStep || undefined"
              @select="stepForward"
            ></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              icon="chevron-double-right"
              text="Naar einde"
              :disabled="activeStep >= maxStep || undefined"
              @click="goToEnd"
            ></nldd-icon-button>
            <nldd-menu-item
              slot="overflow"
              icon="chevron-double-right"
              text="Naar einde"
              :disabled="activeStep >= maxStep || undefined"
              @select="goToEnd"
            ></nldd-menu-item>
          </nldd-toolbar-item>

          <!-- Playback: play/pause. The design system has no media-transport
               glyph, so this is a labelled button; `isPlaying` is the single
               source of truth (a plain button keeps no internal toggle state),
               and the filled `primary` variant signals the playing state. -->
          <nldd-toolbar-item slot="end">
            <nldd-button
              :text="isPlaying ? 'Pauzeren' : 'Afspelen'"
              :appearance="isPlaying ? 'primary' : 'secondary'"
              @click="togglePlay"
            ></nldd-button>
            <nldd-menu-item
              slot="overflow"
              :text="isPlaying ? 'Pauzeren' : 'Afspelen'"
              @select="togglePlay"
            ></nldd-menu-item>
          </nldd-toolbar-item>

          <nldd-toolbar-item slot="end">
            <nldd-tag :text="`${activeStep + 1} / ${maxStep + 1}`"></nldd-tag>
            <!-- The step counter is information, not an action: in the
                 overflow menu it shows as a disabled item. -->
            <nldd-menu-item
              slot="overflow"
              :text="`Stap ${activeStep + 1} van ${maxStep + 1}`"
              disabled
            ></nldd-menu-item>
          </nldd-toolbar-item>

          <!-- Zoom -->
          <nldd-toolbar-item slot="end">
            <nldd-icon-button icon="plus" text="Zoom in" @click="diagramRef?.zoomIn()"></nldd-icon-button>
            <nldd-menu-item slot="overflow" icon="plus" text="Zoom in" @select="diagramRef?.zoomIn()"></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button icon="minus" text="Zoom uit" @click="diagramRef?.zoomOut()"></nldd-icon-button>
            <nldd-menu-item slot="overflow" icon="minus" text="Zoom uit" @select="diagramRef?.zoomOut()"></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button icon="arrow-2-counter-clockwise" text="Centreren" @click="diagramRef?.resetView()"></nldd-icon-button>
            <nldd-menu-item slot="overflow" icon="arrow-2-counter-clockwise" text="Centreren" @select="diagramRef?.resetView()"></nldd-menu-item>
          </nldd-toolbar-item>

          <!-- View mode -->
          <nldd-toolbar-item slot="end">
            <nldd-segmented-control
              size="md"
              width="fit-content"
              :value="viewMode"
              @change="onViewModeChange"
            >
              <nldd-segmented-control-item
                v-for="opt in viewModeOptions"
                :key="opt.value"
                :value="opt.value"
                :text="opt.text"
              ></nldd-segmented-control-item>
            </nldd-segmented-control>
            <nldd-menu-group slot="overflow" text="Weergave">
              <nldd-menu-item
                v-for="opt in viewModeOptions"
                :key="opt.value"
                type="radio"
                :text="opt.text"
                :selected="viewMode === opt.value || undefined"
                @select="opt.value !== viewMode && setViewMode(opt.value)"
              ></nldd-menu-item>
            </nldd-menu-group>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>

      <!-- Main content: the hand-rolled SVG GitFlow canvas (documented design-
           system exception - no nldd equivalent for a graph canvas). -->
      <nldd-full-bleed-section>
        <div class="diagram-area" @click="selectedStageId = null">
          <FlowDiagram
            ref="diagramRef"
            :stages="currentStages"
            :branches="currentBranches"
            :connections="currentConnections"
            :phases="currentPhases"
            :timeline="currentTimeline"
            :active-step="activeStep"
            :selected-id="selectedStageId"
            @select-stage="onSelectStage"
          />
          <FlowLegend />
        </div>
      </nldd-full-bleed-section>
    </nldd-page>

    <!-- Detail panel as a design-system sheet -->
    <StageDetail
      :stage="selectedStage"
      @close="selectedStageId = null"
    />
  </nldd-shell>
</template>

<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue';
import {
  stages as simpleStages,
  branches as simpleBranches,
  connections as simpleConnections,
} from './data/flowDataSimple';
import {
  stages as advancedStages,
  branches as advancedBranches,
  connections as advancedConnections,
  phases as advancedPhases,
} from './data/flowDataAdvanced';
import {
  stages as wooStages,
  branches as wooBranches,
  connections as wooConnections,
  phases as wooPhases,
  timelineMarkers as wooTimeline,
} from './data/flowDataWoo';
import FlowDiagram from './components/FlowDiagram.vue';
import FlowLegend from './components/FlowLegend.vue';
import StageDetail from './components/StageDetail.vue';
import type { FlowDataset } from './data/types';

type ViewMode = 'simple' | 'advanced' | 'woo';

const viewMode = ref<ViewMode>('simple');
const viewModeOptions: { value: ViewMode; text: string }[] = [
  { value: 'simple', text: 'Eenvoudig' },
  { value: 'advanced', text: 'Uitgebreid' },
  { value: 'woo', text: 'Wet open overheid' },
];
const diagramRef = ref<InstanceType<typeof FlowDiagram> | null>(null);

const datasets: Record<ViewMode, FlowDataset> = {
  simple: { stages: simpleStages, branches: simpleBranches, connections: simpleConnections, phases: null, timeline: null },
  advanced: { stages: advancedStages, branches: advancedBranches, connections: advancedConnections, phases: advancedPhases, timeline: null },
  woo: { stages: wooStages, branches: wooBranches, connections: wooConnections, phases: wooPhases, timeline: wooTimeline },
};

const currentData = computed(() => datasets[viewMode.value]);
const currentStages = computed(() => currentData.value.stages);
const currentBranches = computed(() => currentData.value.branches);
const currentConnections = computed(() => currentData.value.connections);
const currentPhases = computed(() => currentData.value.phases);
const currentTimeline = computed(() => currentData.value.timeline);

const maxStep = computed(() => Math.max(0, ...currentStages.value.map((s) => s.step)));
const activeStep = ref(0);
const selectedStageId = ref<string | null>(null);
const isPlaying = ref(false);
let playInterval: ReturnType<typeof setInterval> | null = null;

function isViewMode(value: unknown): value is ViewMode {
  return typeof value === 'string' && Object.hasOwn(datasets, value);
}

function setViewMode(mode: ViewMode) {
  stopPlay();
  viewMode.value = mode;
  activeStep.value = 0;
  selectedStageId.value = null;
}

// nldd-segmented-control emits a `change` CustomEvent carrying the selected
// item value in `detail.value`.
function onViewModeChange(e: Event) {
  const mode = (e as CustomEvent<{ value?: unknown } | undefined>).detail?.value;
  if (isViewMode(mode) && mode !== viewMode.value) setViewMode(mode);
}

const selectedStage = computed(() => {
  if (!selectedStageId.value) return null;
  return currentStages.value.find((s) => s.id === selectedStageId.value) || null;
});

function stepForward() {
  if (activeStep.value < maxStep.value) {
    activeStep.value++;
  } else {
    stopPlay();
  }
}

function stepBack() {
  if (activeStep.value > 0) {
    activeStep.value--;
  }
}

function resetSteps() {
  stopPlay();
  activeStep.value = 0;
  selectedStageId.value = null;
}

function goToEnd() {
  stopPlay();
  activeStep.value = maxStep.value;
}

function togglePlay() {
  if (isPlaying.value) {
    stopPlay();
  } else {
    startPlay();
  }
}

function startPlay() {
  if (activeStep.value >= maxStep.value) {
    activeStep.value = 0;
  }
  isPlaying.value = true;
  stepForward();
  playInterval = setInterval(() => {
    if (activeStep.value >= maxStep.value) {
      stopPlay();
      return;
    }
    stepForward();
  }, 1500);
}

function stopPlay() {
  isPlaying.value = false;
  if (playInterval) {
    clearInterval(playInterval);
    playInterval = null;
  }
}

function onSelectStage(id: string) {
  selectedStageId.value = selectedStageId.value === id ? null : id;
  stopPlay();
}

onUnmounted(() => {
  stopPlay();
});
</script>

<style scoped>
/* Brand logo size in the toolbar title's media slot: the slot caps the height
   to the title group and leaves the size within that to the consumer. */
.app-logo {
  height: 32px;
  width: auto;
  display: block;
}

/* Centre the SVG canvas (the documented design-system exception) inside the
   full-bleed section. */
.diagram-area {
  display: flex;
  flex-direction: column;
  align-items: center;
}
</style>
