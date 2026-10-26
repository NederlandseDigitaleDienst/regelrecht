/**
 * The key hints under the slides, as translatable sentences.
 *
 * A hint is a sentence with keys in it: "← → of Spatie bladeren". The keys are
 * `nldd-keyboard-shortcut` elements, not text, so the sentence cannot go
 * through `v-html` the way the Presentatie tab does it with `<kbd>`. Instead
 * the dictionary holds the whole sentence with a `{placeholder}` per key, and
 * this splits it into text and key segments for the template to render.
 *
 * Whole sentences rather than the words between the keys: a language that puts
 * the verb first ("to browse, press ...") can then do so, and the placeholder
 * check in `i18n.test.ts` guards that no translation drops a key.
 */

/** Which key each placeholder stands for, as `nldd-keyboard-shortcut` spells it. */
export const HINT_KEYS = Object.freeze({
  prev: '←',
  next: '→',
  space: 'Space',
  esc: 'Esc',
  f: 'F',
});

/** A placeholder name in a hint sentence: `prev`, `next`, `space`, `esc` or `f`. */
export type HintKeyName = keyof typeof HINT_KEYS;

/**
 * One piece of a hint sentence: plain text, or a key to render as
 * `nldd-keyboard-shortcut`. The other field is absent, so a template can test
 * `seg.key` and a caller `seg.text` without narrowing first.
 */
export type HintSegment = { text: string; key?: undefined } | { key: string; text?: undefined };

/** The dictionary keys of the hints, in the order they appear. */
export const HINTS = Object.freeze(['deck.hint.browse', 'deck.hint.close', 'deck.hint.fullscreen']);

/**
 * "{esc} sluit" → [{ key: 'Esc' }, { text: ' sluit' }].
 *
 * An unknown placeholder stays as text, braces and all: a typo in a dictionary
 * then shows up on screen instead of silently dropping a word.
 */
export function hintSegments(sentence: unknown): HintSegment[] {
  const text = String(sentence);
  const segments: HintSegment[] = [];
  let last = 0;
  for (const m of text.matchAll(/\{(\w+)\}/g)) {
    // `!`: the pattern has one capture group, and it always takes part.
    const name = m[1]!;
    if (!Object.hasOwn(HINT_KEYS, name)) continue;
    if (m.index > last) segments.push({ text: text.slice(last, m.index) });
    segments.push({ key: HINT_KEYS[name as HintKeyName] });
    last = m.index + m[0].length;
  }
  if (last < text.length) segments.push({ text: text.slice(last) });
  return segments;
}
