<script setup lang="ts">
/**
 * Where the recorded walkthrough starts.
 *
 * On a wide screen this page hands over to the replay (replay.ts): the deck
 * comes up with the presenter's voice and from then on the walkthrough moves
 * through the tabs itself, in the live demo. The page has nothing to show of
 * its own; the first slide covers it.
 *
 * On a phone the demo next to a rail does not fit. The page says so and
 * points to a larger screen; there is no video of the recording to fall back
 * on, since its slides would be the ones of the recording day.
 */
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useDemo } from '../store/demoStore';
import { usePresentation } from '../presentation/usePresentation';
import { useI18n } from '../i18n/index.js';
import { localeRouteName } from '../router';
import { useWalkthrough } from './useWalkthrough';
import { replay, savedPosition, startReplay } from './replay';

/** Below this the demo and the rail do not fit side by side (the deck's own breakpoint). */
const WIDE = 1024;

const { t, locale } = useI18n();
const route = useRoute();
const router = useRouter();
const demo = useDemo();
const presentation = usePresentation();
const { timeline, ready } = useWalkthrough();

const loaded = ref(false);
const narrow = ref(false);
const homePath = computed(() => router.resolve({ name: localeRouteName('home', locale.value) }).path);

// Starting waits for the timeline and the engine; by then the viewer may
// have gone elsewhere, and a walkthrough must not start over another page.
// The tabs are kept alive, so a second visit is an activation, not a mount.
let here = false;

async function maybeStart() {
  here = true;
  await ready;
  await demo.boot().catch(() => {});
  loaded.value = true;
  narrow.value = window.innerWidth < WIDE;
  // A failed load stays failed until a reload: leaving the replay remounts
  // this page, and starting again would fetch the missing file in a loop.
  if (!timeline.value || narrow.value || replay.active || replay.failed) return;
  if (!here || route.meta?.page !== 'rondleiding') return;
  // `:faqId?` is an optional, non-repeatable param: a string or absent, never an array.
  const faqId = route.params.faqId as string | undefined;
  await startReplay(timeline.value, { router, demo, presentation }, { at: savedPosition(), faqId: faqId ?? null });
}

onMounted(maybeStart);
onActivated(maybeStart);
onDeactivated(() => (here = false));
onUnmounted(() => (here = false));
</script>

<template>
  <!-- No walkthrough, or its media did not load (a release asset missing,
       or a dev checkout with a timeline but without `walkthrough build`). -->
  <nldd-page v-if="loaded && (!timeline || replay.failed)">
    <nldd-simple-section width="720px">
      <nldd-title slot="header" size="2">
        <h1>{{ t('walkthrough.missing.title') }}</h1>
        <span slot="supporting-text">{{ t('walkthrough.missing.body') }}</span>
      </nldd-title>
      <nldd-button appearance="primary" start-icon="home" :text="t('walkthrough.leave')" @click="router.push(homePath)"></nldd-button>
    </nldd-simple-section>
  </nldd-page>

  <nldd-page v-else-if="loaded && narrow">
    <nldd-simple-section>
      <nldd-title slot="header" size="2">
        <h1>{{ t('walkthrough.label') }}</h1>
        <span slot="supporting-text">{{ t('walkthrough.phone') }}</span>
      </nldd-title>
      <nldd-button appearance="primary" start-icon="home" :text="t('walkthrough.leave')" @click="router.push(homePath)"></nldd-button>
    </nldd-simple-section>
  </nldd-page>

  <nldd-page v-else>
    <nldd-simple-section height="60vh">
      <nldd-activity-indicator show-text :text="t('app.loading')" timing="instant" size="48"></nldd-activity-indicator>
    </nldd-simple-section>
  </nldd-page>
</template>
