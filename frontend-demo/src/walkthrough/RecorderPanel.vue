<script setup lang="ts">
/**
 * The recorder's controls, shown in dev with `?record` in the address.
 *
 * A take is recorded the way it is played back: in Dutch, with the deck as a
 * rail next to the demo (zelfstandig). The player replays the actions in the
 * live demo, so what matters most is the action log (capture.ts); the video
 * of the window is kept for the phone and for a shareable MP4. The panel
 * leaves the screen while a take runs: the tab title shows that it records,
 * Shift+X marks a slip and Shift+R stops.
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { useDemo } from '../store/demoStore';
import { usePresentation, type PresentationMode } from '../presentation/usePresentation';
import { adoptLocale, currentLocale, useI18n } from '../i18n/index.js';
import { captureActions } from './capture';
import { levelPercent, levelVerdict, listInputs, mic, startTest, stopTest } from './micCheck';
import { cam, lightVerdict, listCameras, startCamTest, stopCamTest } from './camCheck';
import { hasSnapshots, logEvent, markFlub, recorder, rememberSnapshot, snapshotFor, startRecording, stopRecording } from './recorder';

const { t } = useI18n();
const router = useRouter();
const demo = useDemo();
const p = usePresentation();

const startAt = ref(0);
const restore = ref(hasSnapshots());
const slides = computed(() => p.slides.value ?? []);

const cleanups: (() => void)[] = [];
let previousMode: PresentationMode | null = null;
let previousTitle: string | null = null;
let previousLocale: ReturnType<typeof currentLocale> | null = null;

function plain<T>(value: T): T {
  return JSON.parse(JSON.stringify(value));
}

function editable(e: Event) {
  // The path also holds the document and the window, which have no `matches`.
  return (e.composedPath?.() ?? []).some((n) => (n as Partial<Element> | null)?.matches?.('input, textarea, select, [contenteditable]'));
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'X' && e.shiftKey && !editable(e)) {
    e.preventDefault();
    markFlub();
  }
}

function listen() {
  window.addEventListener('keydown', onKey, true);
  cleanups.push(() => window.removeEventListener('keydown', onKey, true));
  cleanups.push(captureActions(logEvent));
  cleanups.push(router.afterEach((to) => logEvent({ type: 'route', path: to.fullPath })));
  cleanups.push(
    watch(
      [p.index, p.active],
      ([index, active]) => {
        if (!active) {
          logEvent({ type: 'deck', active: false });
          return;
        }
        const state = plain(demo.state);
        rememberSnapshot(index, state);
        logEvent({ type: 'slide', index, slide: plain(p.current.value ?? {}), profile: demo.profileKey.value, state });
      },
      { immediate: true },
    ),
  );
}

function unlisten() {
  // The loop condition guarantees pop() returns a function.
  while (cleanups.length) cleanups.pop()!();
}

async function toggleTest() {
  if (mic.testing) stopTest();
  else await startTest().catch((e) => (recorder.error = String(e?.message ?? e)));
}
async function pickInput(id: string) {
  mic.deviceId = id;
  if (mic.testing) await startTest();
}
onMounted(() => listInputs().catch(() => {}));
onUnmounted(stopTest);

const preview = ref<HTMLVideoElement | null>(null);
watch(
  () => [cam.stream, preview.value] as const,
  ([stream, el]) => {
    if (el && el.srcObject !== stream) el.srcObject = stream;
  },
);
async function toggleCamTest() {
  if (cam.testing) stopCamTest();
  else await startCamTest().catch((e) => (recorder.error = String(e?.message ?? e)));
}
async function pickCamera(id: string) {
  cam.deviceId = id;
  if (cam.testing) await startCamTest();
}
onMounted(() => listCameras().catch(() => {}));
onUnmounted(stopCamTest);

async function start() {
  // The tests hold the microphone and camera open; the take opens them again itself.
  stopTest();
  stopCamTest();
  if (restore.value) {
    const snap = snapshotFor(startAt.value);
    if (snap) {
      Object.assign(demo.state, snap);
      demo.reregister();
    }
  }
  previousMode = demo.state.presentationMode;
  demo.state.presentationMode = 'zelfstandig';
  // The replay finds controls by their Dutch text, so a take is Dutch.
  previousLocale = currentLocale();
  adoptLocale('nl');
  const takeId = await startRecording({
    meta: {
      startSlide: startAt.value,
      slideCount: slides.value.length,
      viewport: { width: window.innerWidth, height: window.innerHeight, dpr: window.devicePixelRatio },
      locale: 'nl',
      // The deck as it stood: the player shows these slides, not whatever
      // demo-config.yaml says by the time someone watches.
      slides: plain(p.slides.value ?? []),
      userAgent: navigator.userAgent,
    },
  });
  if (!takeId) {
    restoreAfterTake();
    return;
  }
  // The panel is off screen now; the tab title, outside the picture, says
  // that the take runs.
  previousTitle = document.title;
  document.title = `● REC · ${previousTitle}`;
  // The deck first, then the log: the log's first slide is then the slide the
  // take starts on, not whatever the deck showed a moment before.
  p.start(startAt.value);
  listen();
}

function restoreAfterTake() {
  unlisten();
  if (previousTitle != null) document.title = previousTitle;
  previousTitle = null;
  if (previousMode) demo.state.presentationMode = previousMode;
  previousMode = null;
  if (previousLocale) adoptLocale(previousLocale);
  previousLocale = null;
}

async function stop() {
  await stopRecording();
  restoreAfterTake();
}

// Chrome's own "stop sharing" ends the take from outside this panel.
watch(
  () => recorder.phase,
  (phase) => {
    if (phase === 'saving' || phase === 'error') restoreAfterTake();
  },
);

/** The value of the `<select>` a change event came from. */
function selectValue(e: Event): string {
  return (e.target as HTMLSelectElement).value;
}

function onGlobalKey(e: KeyboardEvent) {
  // Shift+R stops, so the presenter never has to reach for the mouse in the
  // middle of a sentence (and the panel is off screen anyway).
  if (e.key === 'R' && e.shiftKey && !editable(e)) {
    e.preventDefault();
    if (recorder.phase === 'recording') stop();
  }
}
onMounted(() => window.addEventListener('keydown', onGlobalKey));
onUnmounted(() => {
  window.removeEventListener('keydown', onGlobalKey);
  unlisten();
});
</script>

<template>
  <div v-if="recorder.phase !== 'recording'" class="recorder" role="region" :aria-label="t('recorder.label')">
    <nldd-card background="base">
      <nldd-container padding="16" gap="12">
        <template v-if="recorder.phase === 'saving'">
          <nldd-activity-indicator show-text :text="t('recorder.saving')" timing="instant" size="24"></nldd-activity-indicator>
        </template>
        <template v-else>
          <nldd-text size="md">{{ t('recorder.title') }}</nldd-text>
          <nldd-form-field :label="t('recorder.start_at')">
            <nldd-dropdown width="full">
              <select :value="startAt" @change="startAt = Number(selectValue($event))">
                <option v-for="(s, i) in slides" :key="i" :value="i">{{ i + 1 }}. {{ s.title ?? s.lines?.[0]?.replaceAll('**', '') ?? '' }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <!-- The microphone test: pick the input, speak as in the take, read
               the verdict. A take that is too quiet brings its noise along
               when it is turned up afterwards. -->
          <nldd-form-field :label="t('recorder.mic.label')">
            <nldd-dropdown width="full">
              <select :value="mic.deviceId" @change="pickInput(selectValue($event))">
                <option v-for="d in mic.devices" :key="d.id" :value="d.id">{{ d.label }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <nldd-button size="sm" appearance="secondary" :start-icon="mic.testing ? 'stop' : 'speaker'" :text="mic.testing ? t('recorder.mic.stop_test') : t('recorder.mic.test')" @click="toggleTest"></nldd-button>
          <template v-if="mic.testing">
            <nldd-progress-bar size="md" :color="levelVerdict(mic.levelDb, mic.peakDb) === 'recorder.mic.good' ? 'success' : 'warning'" :value="levelPercent(mic.levelDb)" max="100" value-display="none" :accessible-label="t('recorder.mic.level', { db: mic.levelDb })"></nldd-progress-bar>
            <nldd-text size="sm">{{ t(levelVerdict(mic.levelDb, mic.peakDb)) }} ({{ mic.levelDb }} dB)</nldd-text>
          </template>
          <nldd-switch-field :label="t('recorder.camera')" :checked="recorder.withCamera || undefined" @change="recorder.withCamera = !recorder.withCamera"></nldd-switch-field>
          <!-- The camera test: the round cut the viewer will see, to set up
               light and background before the take. -->
          <template v-if="recorder.withCamera">
            <nldd-form-field v-if="cam.devices.length > 1" :label="t('recorder.cam.label')">
              <nldd-dropdown width="full">
                <select :value="cam.deviceId" @change="pickCamera(selectValue($event))">
                  <option v-for="d in cam.devices" :key="d.id" :value="d.id">{{ d.label }}</option>
                </select>
              </nldd-dropdown>
            </nldd-form-field>
            <nldd-button size="sm" appearance="secondary" :start-icon="cam.testing ? 'stop' : 'video-camera'" :text="cam.testing ? t('recorder.cam.stop_test') : t('recorder.cam.test')" @click="toggleCamTest"></nldd-button>
            <template v-if="cam.testing">
              <video ref="preview" class="cam-preview" autoplay muted playsinline :aria-label="t('recorder.cam.preview')"></video>
              <nldd-text size="sm">{{ t(lightVerdict(cam.brightness)) }}</nldd-text>
            </template>
          </template>
          <nldd-switch-field :label="t('recorder.restore')" :checked="restore || undefined" @change="restore = !restore"></nldd-switch-field>
          <nldd-text v-if="recorder.phase === 'done'" size="sm" color="secondary">{{ t('recorder.saved', { take: recorder.takeId }) }}</nldd-text>
          <nldd-text v-if="recorder.error" size="sm" color="critical">{{ recorder.errorKey ? t(recorder.errorKey) : recorder.error }}</nldd-text>
          <nldd-button appearance="primary" start-icon="play" :text="t('recorder.start')" :loading="recorder.phase === 'starting' || undefined" :disabled="!slides.length || undefined" @click="start"></nldd-button>
          <nldd-text size="sm" color="secondary">{{ t('recorder.help') }}</nldd-text>
        </template>
      </nldd-container>
    </nldd-card>
  </div>
</template>

<style scoped>
/* Custom CSS: the camera preview, cut round like the presenter bubble and
   mirrored like a mirror, so moving left moves left. The design system has
   no live video element. */
.cam-preview {
  display: block;
  width: 10rem;
  aspect-ratio: 1;
  margin-inline: auto;
  object-fit: cover;
  border-radius: 50%;
  transform: scaleX(-1);
  background: var(--primitives-color-neutral-900, #111);
}

/* Custom CSS: the design system has no floating panel. Position only; the
   panel itself is an nldd-card. Over the rail, never over the captured area. */
.recorder {
  position: fixed;
  top: 0.75rem;
  left: 0.75rem;
  width: min(22rem, calc(36vw - 1.5rem));
  z-index: 95;
}
</style>
