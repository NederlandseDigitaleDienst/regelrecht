/**
 * A sentence of the pages (UitlegPagina.vue, KortPagina.vue) in pieces: plain
 * text, and `name` as code, as in the deck (slides.js).
 */
export const stukken = (zin) =>
  String(zin)
    .split(/(`[^`]+`)/)
    .filter(Boolean)
    .map((s) => (s.startsWith('`') ? { code: s.slice(1, -1) } : { tekst: s }));
