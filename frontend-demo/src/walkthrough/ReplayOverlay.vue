<script setup lang="ts">
/**
 * What lies over the live demo while a walkthrough runs: the presenter in a
 * circle, the captions, the cursor that shows where the presenter clicks, and
 * the transcript panel. Also the player's keys.
 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { useViewerI18n } from '../i18n/index.js';
import { usePresentation } from '../presentation/usePresentation';
import { PLAYER_KEYS, audioTime, backToMain, chapterSlide, currentTrack, questionOf, replay, seek, togglePlay } from './replay';
import { MEDIA_BASE, camVisible, cueAt, followerCorrection, formatTime, nextChapterStart, previousChapterStart } from './timeline';
import { camSlot, transcript } from './chrome';

const { t, locale } = useViewerI18n();
const p = usePresentation();

const track = computed(() => currentTrack());
// While a jump restores the demo, the clock runs through the chapter at speed:
// the captions and the presenter would flicker along, so they wait under the cover.
const cue = computed(() => (replay.captionsOn && !replay.restoring ? cueAt(replay.cues, replay.now) : null));
const full = computed(() => p.isFull.value);
/**
 * A full slide with the presenter on camera (the opening, or an answer with
 * no demo in it): then the face is what there is to see, so the bubble is
 * large, next to the slide, instead of a corner of the screen. Once a slide
 * opens the demo it shrinks back.
 */
const face = computed(() => full.value && camVisible(track.value, replay.now));
// The deck lives outside this component; a class on <html> lets its slide
// make room for the face (presentation.css).
watch(face, (on) => document.documentElement.classList.toggle('rr-face', on), { immediate: true });
onUnmounted(() => document.documentElement.classList.remove('rr-face'));

// ---- the bubble -------------------------------------------------------------------

const cam = ref<HTMLVideoElement | null>(null);
const camStyle = ref<Record<string, string>>({});

/** Lay the bubble over its spot in the rail; on a full slide the stylesheet places it. */
function placeCam() {
  const r = camSlot.value?.getBoundingClientRect?.();
  camStyle.value = !full.value && r && r.width > 0 ? { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` } : {};
}

let raf = 0;
let lastSync = 0;
function loop() {
  const c = cam.value;
  if (c && replay.restoring && !c.paused) c.pause();
  if (c && replay.camOn && !replay.restoring && camVisible(track.value, replay.now) && performance.now() - lastSync > 250) {
    lastSync = performance.now();
    const fix = followerCorrection(audioTime(), c.currentTime, replay.speed);
    if (fix.seek != null) c.currentTime = fix.seek;
    c.playbackRate = fix.rate;
    if (replay.playing && !replay.waiting && c.paused) c.play().catch(() => {});
    if ((!replay.playing || replay.waiting) && !c.paused) c.pause();
  }
  placeCam();
  raf = requestAnimationFrame(loop);
}

// ---- the top layer ---------------------------------------------------------------------

const pointerLayer = ref<HTMLElement | null>(null);
function raise() {
  const el = pointerLayer.value;
  if (!el?.showPopover) return;
  try {
    if (el.matches(':popover-open')) el.hidePopover();
    el.showPopover();
  } catch {
    /* not supported: the layer stays in normal stacking */
  }
}
watch(() => [replay.cursor.x, replay.cursor.y, replay.ripples.length, cue.value?.text], raise);
onMounted(raise);

// The cover while a jump restores the demo: a layer of its own, because the
// pointer layer is aria-hidden and its status message has to be heard. Raised
// again as the restore goes on, above a sheet or dialog it opens.
const restoreLayer = ref<HTMLElement | null>(null);
function raiseCover() {
  const el = restoreLayer.value;
  if (!el?.showPopover) return;
  try {
    if (el.matches(':popover-open')) {
      // Already on top: leave it, or a screen reader hears the status again.
      const r = el.getBoundingClientRect();
      if (el.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2))) return;
      el.hidePopover();
    }
    el.showPopover();
  } catch {
    /* not supported: the cover stays in normal stacking */
  }
}
watch(
  () => replay.restoring && replay.now,
  () => replay.restoring && nextTick(raiseCover),
);

// ---- the transcript ------------------------------------------------------------------

const sheet = ref<NLDDSheet | null>(null);
watch(
  () => transcript.open,
  (open) => (open ? sheet.value?.show?.() : sheet.value?.hide?.()),
);
const chapters = computed(() =>
  (track.value?.chapters ?? []).map((c, i, all) => {
    const end = all[i + 1]?.start ?? Infinity;
    return {
      start: c.start,
      title: chapterSlide(track.value, c)?.title ?? chapterSlide(track.value, c)?.lines?.[0]?.replaceAll('**', '') ?? t('walkthrough.chapter', { n: i + 1 }),
      text: replay.cues
        .filter((q) => q.start >= c.start && q.start < end)
        .map((q) => q.text.replace(/\n/g, ' '))
        .join(' '),
    };
  }),
);
function playFrom(at: number) {
  transcript.open = false;
  seek(at, { play: true });
}

// ---- keys ------------------------------------------------------------------------------

function pathHas(e: Event, selector: string) {
  // The path also holds the document and the window, which have no `matches`.
  return (e.composedPath?.() ?? []).some((n) => (n as Partial<Element> | null)?.matches?.(selector));
}

function onKey(e: KeyboardEvent) {
  if (!replay.active || e.metaKey || e.ctrlKey || e.altKey || transcript.open) return;
  if (!PLAYER_KEYS.has(e.key)) return;
  // A menu keeps its keys.
  if (pathHas(e, 'nldd-menu, nldd-menu-item, [role="menu"]')) return;
  // A field keeps them too, unless the walkthrough is playing: then focus in a
  // field is where the replay typed, and Space must pause rather than type.
  if (!replay.playing && pathHas(e, 'input, textarea, select, [contenteditable]')) return;
  if (replay.diverged && !pathHas(e, '.deck, .wt-chrome')) return;
  if (e.key === ' ' && pathHas(e, 'button, a[href], nldd-button, nldd-icon-button')) return;
  // Ours: the field or the deck underneath must not act on it as well.
  e.stopPropagation();
  const tr = track.value;
  switch (e.key) {
    // No single-letter shortcuts (WCAG 2.1.4): a speech-input user saying a
    // word would fire them. Space, the arrows and Escape are enough.
    case ' ':
      e.preventDefault();
      togglePlay();
      break;
    case 'ArrowLeft':
      e.preventDefault();
      seek(previousChapterStart(tr, replay.now));
      break;
    case 'ArrowRight': {
      e.preventDefault();
      const to = nextChapterStart(tr, replay.now);
      if (to != null) seek(to);
      break;
    }
    case 'Escape':
      if (replay.faq) backToMain();
      break;
    default:
  }
}

onMounted(() => {
  raf = requestAnimationFrame(loop);
  // Capture phase: ahead of a field that has focus.
  window.addEventListener('keydown', onKey, true);
});
onUnmounted(() => {
  cancelAnimationFrame(raf);
  window.removeEventListener('keydown', onKey, true);
});
</script>

<template>
  <!-- In <body>, like the deck: inside the workspace this layer would take
       part in its layout and push the demo down. -->
  <Teleport to="body">
  <div class="wt-chrome overlay" :class="{ full, face }">
    <video
      v-if="track?.cam"
      v-show="replay.camOn && !replay.restoring && camVisible(track, replay.now)"
      ref="cam"
      class="cam"
      :style="camStyle"
      :src="`${MEDIA_BASE}${track.cam.src}`"
      muted
      playsinline
      preload="auto"
      aria-hidden="true"
    ></video>

    <!-- The pointer and the captions go in the browser's top layer, as a
         popover: a sheet or dialog the replay opens lives there too, and
         would otherwise cover the cursor exactly when it clicks inside it.
         Raised again whenever the cursor moves, so it stays on top of a
         dialog that opened after it. -->
    <div ref="pointerLayer" popover="manual" class="pointer-layer" aria-hidden="true">
    <svg v-show="replay.cursor.visible && !replay.diverged && !replay.restoring" class="cursor" :style="{ transform: `translate(${replay.cursor.x}px, ${replay.cursor.y}px)` }" width="22" height="26" viewBox="0 0 22 26" aria-hidden="true">
      <path d="M2 2 L2 20 L7 15.5 L10.5 23.5 L13.5 22.2 L10 14.4 L17 14.4 Z" />
    </svg>
    <span v-for="r in replay.restoring ? [] : replay.ripples" :key="r.id" class="ripple" :style="{ left: `${r.x}px`, top: `${r.y}px` }" aria-hidden="true"></span>

    <p v-if="cue" class="caption" :lang="replay.captionsLang" aria-hidden="true">{{ cue.text }}</p>
    </div>
    <div v-if="replay.restoring" ref="restoreLayer" popover="manual" class="restoring">
      <nldd-activity-indicator show-text :text="t('walkthrough.restoring')" timing="instant" size="48"></nldd-activity-indicator>
    </div>

    <nldd-sheet ref="sheet" placement="right" width="560px" :accessible-label="t('walkthrough.transcript.title')" @close="transcript.open = false">
      <nldd-page>
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="t('walkthrough.transcript.title')" :supporting-text="replay.faq ? questionOf(replay.faq) : undefined" :dismiss-text="t('walkthrough.transcript.close')" @dismiss="transcript.open = false"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="24" :lang="replay.captionsLang">
          <nldd-container v-for="(c, i) in chapters" :key="i" padding="0" gap="8">
            <nldd-title size="5">
              <h2>{{ c.title }}</h2>
            </nldd-title>
            <nldd-rich-text v-if="c.text" spacing="tight">
              <p>{{ c.text }}</p>
            </nldd-rich-text>
            <nldd-button size="sm" appearance="neutral-transparent" start-icon="play" :lang="locale" :text="t('walkthrough.transcript.play_from', { time: formatTime(c.start) })" @click="playFrom(c.start)"></nldd-button>
          </nldd-container>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </div>
  </Teleport>
</template>

<style scoped>
/* Custom CSS, on purpose: the design system has no picture-in-picture bubble,
 * no caption overlay and no pointer. The panel is an nldd-sheet. */
.cam {
  position: fixed;
  z-index: 85;
  width: 240px;
  height: 240px;
  left: calc(100vw - 240px - 4rem);
  top: calc(100vh - 240px - 12rem);
  box-sizing: border-box;
  border-radius: 50%;
  object-fit: cover;
  box-shadow: 0 6px 24px rgb(0 0 0 / 0.35);
  /* Fixed white, not a primitive: those flip to black in dark mode. */
  border: 3px solid #fff;
  transition: left 0.5s cubic-bezier(0.22, 1, 0.36, 1), top 0.5s cubic-bezier(0.22, 1, 0.36, 1), width 0.5s, height 0.5s;
  pointer-events: none;
}
/* The presenter talking, in an answer without demo: large, right of the
   question. Left/top/size only, so the change animates like the rest. */
.face .cam {
  width: min(46vh, 30vw);
  height: min(46vh, 30vw);
  left: calc(100vw - min(46vh, 30vw) - 8vw);
  top: calc(50vh - min(46vh, 30vw) / 2 - 4vh);
}
/* The popover's own box is reset to nothing; its children are fixed to the
   viewport like the rest of the overlay. */
.pointer-layer {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  margin: 0;
  padding: 0;
  border: 0;
  background: transparent;
  overflow: visible;
  pointer-events: none;
}
.cursor {
  position: fixed;
  left: 0;
  top: 0;
  z-index: 95;
  margin: -2px 0 0 -2px;
  pointer-events: none;
  transition: transform 0.35s cubic-bezier(0.22, 1, 0.36, 1);
}
.cursor path {
  /* A pointer is black with a white edge in either color scheme. */
  fill: #000;
  stroke: #fff;
  stroke-width: 1.5;
}
.ripple {
  position: fixed;
  z-index: 94;
  width: 44px;
  height: 44px;
  margin: -22px 0 0 -22px;
  border-radius: 50%;
  border: 3px solid var(--primitives-color-donkergeel-200);
  pointer-events: none;
  animation: rr-ripple 0.7s ease-out forwards;
}
@keyframes rr-ripple {
  from {
    transform: scale(0.3);
    opacity: 1;
  }
  to {
    transform: scale(1.4);
    opacity: 0;
  }
}
/* While a jump puts the demo back: the demo is covered, the rail is not.
   A popover, so the browser's own popover box is reset like the pointer layer. */
.restoring {
  position: fixed;
  inset: 0 0 0 36vw;
  width: auto;
  height: auto;
  margin: 0;
  padding: 0;
  border: 0;
}
.restoring:popover-open {
  display: grid;
  place-items: center;
  background: var(--semantics-surfaces-base-background-color);
  pointer-events: auto;
}
.overlay.full .restoring {
  inset: 0;
}
.caption {
  position: fixed;
  z-index: 86;
  left: calc(36vw + 2rem);
  right: 2rem;
  bottom: 2rem;
  margin: 0 auto;
  width: fit-content;
  max-width: 60ch;
  padding: 0.4rem 0.8rem;
  border-radius: 6px;
  background: rgb(0 0 0 / 0.78);
  /* Fixed white on the fixed black box: the primitive tokens are light-dark()
     pairs and gave black on black in dark mode. */
  color: #fff;
  font-family: 'RijksSans', system-ui, sans-serif;
  font-size: clamp(1rem, 1.4vw, 1.4rem);
  line-height: 1.35;
  text-align: center;
  white-space: pre-line;
  pointer-events: none;
}
.full .caption {
  left: 2rem;
  bottom: 12rem;
}
@media (prefers-reduced-motion: reduce) {
  .cam,
  .cursor {
    transition: none;
  }
  .ripple {
    animation-duration: 0.01s;
  }
}
</style>
