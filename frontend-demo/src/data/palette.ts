/**
 * The colours the demo gives to things that need telling apart: a law in the
 * trace, a run in the simulation. Rijkshuisstijl primitives, in a fixed order,
 * handed out in the order the things appear, so two of them on one screen
 * never share a colour (a hash of a name would).
 *
 * The primitives flip in the dark scheme, so one step reads in both.
 */
export const PALETTE: readonly string[] = ['hemelblauw', 'oranje', 'paars', 'groen', 'robijnrood', 'donkergeel', 'mintgroen', 'bruin', 'violet', 'donkerblauw'];

/** The CSS expression for colour `i` of the palette at `step` (500 by default). */
export function paletteColor(i: number, step: number = 500): string {
  return `var(--primitives-color-${PALETTE[i % PALETTE.length]}-${step})`;
}

/** The colour of something with no colour of its own: a baseline, a default. */
export const NEUTRAL = 'var(--primitives-color-coolgray-400)';
