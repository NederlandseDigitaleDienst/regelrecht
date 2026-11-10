import type { LawDocument } from './lawFormat';

/**
 * Human-readable fallback for a law id, used when the real name isn't available
 * yet (corpus not loaded, or the law isn't in the active corpus):
 * `algemene_wet_inkomensafhankelijke_regelingen`
 *   → "Algemene Wet Inkomensafhankelijke Regelingen".
 *
 * Title-cased to match the library's existing display; callers prefer the real
 * `name` and only fall back to this.
 */
export function humanizeLawId(id: unknown): string {
  return String(id ?? '').replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}

/**
 * A rulework's display name: its `name`, or, when that is a reference like
 * `#wet_naam`, the literal string the referenced action outputs. Without a
 * name, or with a reference that does not resolve to a string, it is a
 * readable version of the id: the raw `#wet_naam` is never a name.
 */
export function resolveLawName(law: LawDocument | null | undefined): string {
  if (!law) return '';
  const nameRef = law.name;
  if (typeof nameRef === 'string' && nameRef.startsWith('#')) {
    const outputName = nameRef.slice(1);
    for (const article of law.articles ?? []) {
      const actions = article.machine_readable?.execution?.actions;
      if (!actions) continue;
      for (const action of actions) {
        // The schema allows any operation value here; only a literal string
        // is a name.
        if (action.output === outputName && typeof action.value === 'string') return action.value;
      }
    }
    return humanizeLawId(law.$id);
  }
  // No explicit name → a readable version of the id, never the raw snake_case
  // (e.g. tab labels / titles for laws whose YAML carries no `name`).
  return nameRef || humanizeLawId(law.$id);
}
