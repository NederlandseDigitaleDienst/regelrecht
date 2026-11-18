/**
 * Whether this build carries a recorded walkthrough, loaded once and shared.
 *
 * The home page and the presentation page offer the walkthrough only when
 * there is one; a deploy without the media simply shows no button.
 */
import { shallowRef } from 'vue';
import { loadTimeline } from './timeline';
import type { Timeline } from './timeline';

const timeline = shallowRef<Timeline | null>(null);
let pending: Promise<Timeline | null> | null = null;

export function useWalkthrough() {
  if (!pending) {
    pending = loadTimeline().then((t) => {
      timeline.value = t;
      return t;
    });
  }
  return { timeline, ready: pending };
}
