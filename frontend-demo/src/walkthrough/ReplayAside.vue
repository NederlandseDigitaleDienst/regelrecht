<script setup lang="ts">
/**
 * Under the slide in the rail, while a walkthrough runs: the spot for the
 * presenter's bubble and the questions offered so far.
 */
import { computed, onUnmounted, ref, watch } from 'vue';
import { useViewerI18n } from '../i18n/index.js';
import { useWalkthrough } from './useWalkthrough';
import { currentTrack, openFaq, questionLang, questionOf, replay } from './replay';
import { camVisible, offeredFaq } from './timeline';
import { camSlot } from './chrome';

const { t, locale } = useViewerI18n();
const { timeline } = useWalkthrough();

const slot = ref<HTMLElement | null>(null);
watch(slot, (el) => (camSlot.value = el), { immediate: true });
onUnmounted(() => (camSlot.value = null));

const showCam = computed(() => !replay.dubbed && camVisible(currentTrack(), replay.now) && replay.camOn);
const offered = computed(() => (replay.faq ? [] : offeredFaq(timeline.value, replay.now)));
</script>

<template>
  <div v-if="showCam || offered.length" class="aside wt-chrome" :lang="locale">
    <div v-if="showCam" ref="slot" class="cam-slot" aria-hidden="true"></div>
    <div v-if="offered.length" class="faq" role="region" :aria-label="t('walkthrough.faq.label')">
      <span class="faq-title" :lang="locale">{{ t('walkthrough.faq.title') }}</span>
      <TransitionGroup name="faq" tag="div" class="faq-items">
        <nldd-button
          v-for="f in offered"
          :key="f.id"
          size="sm"
          appearance="inherit-tinted"
          start-icon="question"
          horizontal-alignment="left"
          width="full"
          :text="questionOf(f)"
          :lang="questionLang(f)"
          @click="openFaq(f)"
        ></nldd-button>
      </TransitionGroup>
    </div>
  </div>
</template>

<style scoped>
/* Custom CSS: a reserved spot for the bubble and the list of questions
   appearing. The questions themselves are nldd buttons. */
.aside {
  display: flex;
  align-items: flex-end;
  gap: 1.25rem;
  margin-top: 1rem;
}
.cam-slot {
  flex: 0 0 auto;
  width: 120px;
  height: 120px;
}
.faq {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.faq-title {
  font-size: 0.85rem;
  font-weight: 600;
  opacity: 0.72;
}
.faq-items {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}
.faq-enter-active {
  transition: opacity 0.6s ease, transform 0.6s ease;
}
.faq-enter-from {
  opacity: 0;
  transform: translateY(6px);
}
@media (prefers-reduced-motion: reduce) {
  .faq-enter-active {
    transition: none;
  }
}
</style>
