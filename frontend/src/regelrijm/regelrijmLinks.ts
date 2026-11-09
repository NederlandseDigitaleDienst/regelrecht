// Which fragments of the article text a rule comes from. Only what is recorded
// counts: a linking note (RFC-005, `motivation: linking`) anchors a fragment
// of the text and names the rule in its body as `regelrecht://<law>/<output>`.
// Nothing here guesses a link from matching words.
import { cpToUtf16 } from '../composables/useTextSelection';
import { quoteContext } from '../lib/quoteContext';

// The editor's note form writes `regelrecht://<law>/<output>#<output>`; the
// corpus writes it without the fragment. Both name the same element.
const LINK = /^regelrecht:\/\/([^/#?]+)\/([^/#?]+)(?:#[^#?]*)?$/;

/** A fragment of the article text linked to a rule: a quoteContext() result and where it starts. */
export type LinkedFragment = { start: number } & ReturnType<typeof quoteContext>;

/** A resolved note for the article, as useNotes gives it: spans in code points. */
export interface NoteSpans {
  note: unknown;
  spans?: readonly { start: number; end: number }[] | null;
}

// A field of an object, or undefined when `v` is no object.
const field = (v: unknown, key: string): unknown => (v !== null && typeof v === 'object' ? (v as Record<string, unknown>)[key] : undefined);

/**
 * The output a note links to in law `lawId` (one id, or several that name the
 * same law: the editor's note form writes the route id, the corpus the
 * `$id`), or null when it carries no such
 * link. A note can combine a comment with a link (its motivation is then
 * `commenting`), so the link is read from a body with purpose `linking`, or
 * from an untagged body of a `linking` note.
 */
export function linkedOutput(note: unknown, lawId: unknown): string | null {
  if (!note) return null;
  const ids: unknown[] = (Array.isArray(lawId) ? lawId : [lawId]).filter(Boolean);
  const noteBody = field(note, 'body');
  const bodies: unknown[] = Array.isArray(noteBody) ? noteBody : [noteBody];
  for (const body of bodies) {
    if (!body || typeof body !== 'object') continue;
    const purpose = field(body, 'purpose');
    const linking = purpose ? purpose === 'linking' : field(note, 'motivation') === 'linking';
    if (!linking) continue;
    const source = field(body, 'source');
    const m = typeof source === 'string' ? source.match(LINK) : null;
    // Both groups of LINK always take part in a match.
    if (m && ids.includes(m[1])) return m[2]!;
  }
  return null;
}

/**
 * Output → the fragments of `text` linked to it, each a quoteContext()
 * result, in text order. `entries` are resolved notes for the article
 * (`{ note, spans }`, spans in code points, as useNotes gives them).
 */
export function linksByOutput(
  entries: readonly NoteSpans[] | null | undefined,
  { lawId, text }: { lawId: unknown; text: string },
): Map<string, LinkedFragment[]> {
  const byOutput = new Map<string, Map<string, LinkedFragment>>();
  for (const { note, spans } of entries ?? []) {
    const output = linkedOutput(note, lawId);
    if (!output) continue;
    for (const span of spans ?? []) {
      const start = cpToUtf16(text, span.start);
      const end = cpToUtf16(text, span.end);
      let frags = byOutput.get(output);
      if (!frags) byOutput.set(output, (frags = new Map()));
      // Two notes on one fragment show it once.
      frags.set(`${start}-${end}`, { start, ...quoteContext(text, start, end) });
    }
  }
  const out = new Map<string, LinkedFragment[]>();
  for (const [output, frags] of byOutput) out.set(output, [...frags.values()].sort((a, b) => a.start - b.start));
  return out;
}
