// How a reference reads. A name with a term reads as `‹term›`; a name without
// one is humanised and reads as `[naam]`, so it is visible and countable
// without knowing any vocabulary. Every name is delimited, which keeps the
// reading unambiguous whatever words a term contains (RFC-054).

/**
 * Engine context variables. Generic, not tied to any corpus.
 */
import type { Glossary, Vocabulary } from './types';

export const BUILTIN_TERMS: Readonly<Record<string, string>> = Object.freeze({
  referencedate: 'de peildatum',
});

const FORBIDDEN = /[‹›[\]“”]/;

/**
 * Humanising only replaces `_` with a space. Case is kept, so the step is
 * reversible: `PERCENTAGE_LID_5` and `percentage_lid_5` stay different names.
 */
export function humanizeName(name: string): string {
  return name.replace(/_/g, ' ');
}

/**
 * A vocabulary from a glossary `{ name: term }`. Built-in terms come first;
 * the glossary may override them. Throws when two names share a term or a
 * term contains a delimiter, because either would make two trees read alike.
 */
export function makeVocabulary(glossary: Glossary | null = {}): Vocabulary {
  const terms = new Map<string, string>(Object.entries(BUILTIN_TERMS));
  for (const [name, term] of Object.entries(glossary ?? {})) {
    if (typeof term !== 'string' || term.trim() === '') continue;
    terms.set(name, term);
  }
  const names = new Map<string, string>();
  for (const [name, term] of terms) {
    if (FORBIDDEN.test(term)) throw new Error(`RegelRijm: term voor '${name}' bevat een scheidingsteken: ${term}`);
    if (term !== term.trim() || /\s\s/.test(term)) throw new Error(`RegelRijm: term voor '${name}' heeft overbodige witruimte`);
    if (names.has(term)) throw new Error(`RegelRijm: '${names.get(term)}' en '${name}' hebben dezelfde term '${term}'`);
    names.set(term, name);
  }
  return {
    termOf: (name: string) => terms.get(name) ?? null,
    nameOf: (term: string) => names.get(term) ?? null,
  };
}

/** The segments of a reference, deepest first: `$a.b.c` reads `c van b van a`. */
export function referenceSegments(ref: string): string[] {
  return ref.slice(1).split('.').reverse();
}
