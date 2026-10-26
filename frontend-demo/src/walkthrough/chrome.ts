/**
 * State the pieces of the player share: the spot in the rail where the
 * webcam bubble goes, and whether the transcript is open. The pieces live in
 * different places (the deck's slots, an overlay next to it), so they meet
 * here rather than through props.
 */
import { reactive, shallowRef } from 'vue';

/** The empty element in the rail the bubble is laid over. */
export const camSlot = shallowRef<Element | null>(null);

export const transcript = reactive({ open: false });

export function openTranscript() {
  transcript.open = true;
}
