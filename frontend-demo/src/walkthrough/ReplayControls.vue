<script setup lang="ts">
/**
 * The walkthrough's controls, in the deck's footer while a replay runs.
 *
 * What a viewer reaches for (play, chapters) is a button; speed, captions,
 * the transcript, the bubble and leaving sit under one menu. Below them one
 * button that depends on the moment: back out of a question, or "try it
 * yourself", which simply hands the live demo to the viewer.
 */
import { computed, nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { DEFAULT_LOCALE, useViewerI18n } from '../i18n/index.js';
import { intlLocale } from '../data/format';
import { localeRouteName } from '../router';
import { backToMain, chapterSlide, currentTrack, replay, seek, setSpeed, stopReplay, takeOver, togglePlay } from './replay';
import { chapterAt, formatTime, nextChapterStart, previousChapterStart } from './timeline';
import { openTranscript } from './chrome';

const { t, locale } = useViewerI18n();
const router = useRouter();

const SPEEDS = [1, 1.25, 1.5];
function speedText(s: number) {
  return `${new Intl.NumberFormat(intlLocale()).format(s)}×`;
}

const track = computed(() => currentTrack());
const chapters = computed(() => track.value?.chapters ?? []);
const chapterIndex = computed(() => (track.value ? chapterAt(track.value, replay.now) : -1));
const progress = computed(() => (track.value?.duration ? Math.min(100, (replay.now / track.value.duration) * 100) : 0));
const timeText = computed(() => `${formatTime(replay.now)} / ${formatTime(track.value?.duration ?? 0)}`);

function prevChapter() {
  seek(previousChapterStart(track.value, replay.now));
}
function nextChapter() {
  const to = nextChapterStart(track.value, replay.now);
  if (to != null) seek(to);
}
/** A sentence again, or past one: ten seconds back or forward. */
const STEP = 10;
function skip(by: number) {
  const end = track.value?.duration ?? 0;
  seek(Math.max(0, Math.min(end - 0.5, replay.now + by)));
}
function tryIt() {
  // The demo is live; pausing is all it takes. Play brings the recorded
  // moment back.
  takeOver();
}

// The hint at the play button while the player waits for the viewer: at a
// moment the presenter invites them to look around, or when they took over.
const invite = computed(() =>
  replay.invite === 'pause'
    ? { title: t('walkthrough.invite.pause.title'), body: replay.inviteHint ?? t('walkthrough.invite.pause.body') }
    : replay.invite === 'wait'
      ? { title: t('walkthrough.invite.wait.title'), body: t('walkthrough.invite.wait.body') }
      : null,
);
const playText = computed(() => (replay.playing ? t('walkthrough.pause') : replay.invite ? t('walkthrough.continue') : t('walkthrough.play')));
async function leave() {
  // Away from the walkthrough's page first: stopping remounts the tabs, and
  // on that page a fresh mount would start the walkthrough again.
  await router.push(router.resolve({ name: localeRouteName('home', locale.value) }).path);
  stopReplay();
}

const backButton = ref<HTMLElement | null>(null);
const playButton = ref<HTMLElement | null>(null);
// When a question opens, the question that had focus is gone from the rail;
// hand focus to the way back, and to play on return.
watch(
  () => replay.faq,
  (f, old) => nextTick(() => (f ? backButton.value : old ? playButton.value : null)?.focus?.()),
);
</script>

<template>
  <div class="controls wt-chrome" :lang="locale">
    <!-- The voice is Dutch in every interface language. The captions, the
         slides and the demo follow the viewer where the pipeline translated
         them (timeline.json); otherwise they are Dutch, and this says so. -->
    <nldd-text v-if="locale !== DEFAULT_LOCALE" size="sm" color="inherit">{{
      replay.captionsLang === locale && replay.demoLocale === locale ? t('walkthrough.dutch_voice') : t('walkthrough.dutch_only')
    }}</nldd-text>
    <!-- Generated speech that passes for a person is labelled as such (AI
         Act, art. 50). Always in view, not only on the first slide: a viewer
         can start at any chapter. -->
    <nldd-text v-if="track?.generatedVoice" size="sm" color="inherit">{{ t('walkthrough.ai_voice') }}</nldd-text>
    <nldd-progress-bar size="sm" color="donkergeel" :value="progress" max="100" value-display="none" :accessible-label="t('walkthrough.progress_at', { time: timeText })"></nldd-progress-bar>
    <div class="control-row">
      <!-- The coach mark points at the playback buttons while the player
           waits: the viewer may click around, and this is the way back.
           Not dismissable: it goes when play is pressed. -->
      <nldd-just-in-time-education :active="!!invite || undefined" :text="invite?.title" :supporting-text="invite?.body" placement="top">
        <nldd-button-bar>
          <nldd-icon-button appearance="inherit-tinted" icon="media-backward-end" :text="t('walkthrough.prev_chapter')" @click="prevChapter"></nldd-icon-button>
          <nldd-icon-button appearance="inherit-tinted" icon="media-backward" :text="t('walkthrough.back_seconds', { n: STEP })" @click="skip(-STEP)"></nldd-icon-button>
          <nldd-icon-button ref="playButton" appearance="inherit-filled" :icon="replay.playing ? 'pause' : 'play'" :text="playText" @click="togglePlay"></nldd-icon-button>
          <nldd-icon-button appearance="inherit-tinted" icon="media-forward" :text="t('walkthrough.forward_seconds', { n: STEP })" @click="skip(STEP)"></nldd-icon-button>
          <nldd-icon-button appearance="inherit-tinted" icon="media-forward-end" :text="t('walkthrough.next_chapter')" @click="nextChapter"></nldd-icon-button>
        </nldd-button-bar>
      </nldd-just-in-time-education>
      <span class="time" aria-hidden="true">{{ timeText }}</span>
      <nldd-button-bar>
        <nldd-button appearance="inherit-tinted" size="sm" :text="t('walkthrough.chapters')" expandable popup-type="menu">
          <nldd-menu slot="popup" placement="top-start" :accessible-label="t('walkthrough.chapters')">
            <nldd-menu-item
              v-for="(c, i) in chapters"
              :key="i"
              type="radio"
              :text="chapterSlide(track, c)?.title ?? chapterSlide(track, c)?.lines?.[0]?.replaceAll('**', '') ?? t('walkthrough.chapter', { n: i + 1 })"
              :details="formatTime(c.start)"
              :selected="i === chapterIndex || undefined"
              @select="seek(c.start)"
            ></nldd-menu-item>
          </nldd-menu>
        </nldd-button>
        <nldd-icon-button appearance="inherit-tinted" icon="more" :text="t('walkthrough.more')" expandable popup-type="menu">
          <nldd-menu slot="popup" placement="top-end" :accessible-label="t('walkthrough.more')">
            <!-- `@select` on each item, not on the group: the same reason as
                 the toolbar menus in App.vue. -->
            <nldd-menu-group :text="t('walkthrough.speed')">
              <nldd-menu-item v-for="s in SPEEDS" :key="s" type="radio" :text="speedText(s)" :selected="s === replay.speed || undefined" @select="setSpeed(s)"></nldd-menu-item>
            </nldd-menu-group>
            <nldd-menu-group :text="t('walkthrough.view')">
              <nldd-menu-item type="checkbox" icon="message-rectangle-text" :text="t('walkthrough.captions')" :selected="replay.captionsOn || undefined" @select="replay.captionsOn = !replay.captionsOn"></nldd-menu-item>
              <nldd-menu-item v-if="track?.cam" type="checkbox" icon="person-circle" :text="t('walkthrough.camera')" :selected="replay.camOn || undefined" @select="replay.camOn = !replay.camOn"></nldd-menu-item>
              <nldd-menu-item icon="text-document" :text="t('walkthrough.transcript.open')" @select="openTranscript"></nldd-menu-item>
            </nldd-menu-group>
            <nldd-menu-group>
              <nldd-menu-item icon="home" :text="t('walkthrough.leave')" @select="leave"></nldd-menu-item>
            </nldd-menu-group>
          </nldd-menu>
        </nldd-icon-button>
      </nldd-button-bar>
    </div>
    <nldd-button v-if="replay.faq" ref="backButton" appearance="inherit-filled" size="sm" start-icon="back" :text="t('walkthrough.faq.back')" @click="backToMain"></nldd-button>
    <nldd-text v-else-if="replay.diverged" size="sm" color="inherit">{{ t('walkthrough.own_turn') }}</nldd-text>
    <nldd-button v-else appearance="inherit-tinted" size="sm" start-icon="hand" :text="t('walkthrough.try')" @click="tryIt"></nldd-button>
  </div>
</template>

<style scoped>
/* Layout only; the controls are nldd components. Custom CSS: the deck has no
   player footer of its own. */
.controls {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  padding-top: 1rem;
}
.controls > nldd-button {
  align-self: flex-start;
}
.control-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem 1rem;
}
.time {
  font-variant-numeric: tabular-nums;
  font-size: 0.9rem;
  opacity: 0.8;
}
</style>
