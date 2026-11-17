/**
 * The versions of one regelwerk, as the corpus hands them over: one YAML text
 * per version, each with its own top-level `valid_from`.
 */

// Top-level only: an article or an action can carry a valid_from of its own.
const VALID_FROM = /^valid_from:\s*['"]?(\d{4}-\d{2}-\d{2})/m;

/** The valid_from of a version's YAML; null when it has none. */
export function versionValidFrom(yaml: string): string | null {
  return VALID_FROM.exec(yaml)?.[1] ?? null;
}

/**
 * The saved versions other than the one with `validFrom`: the version being
 * edited goes into the engine as edited, and its saved copy must not stay
 * next to it under its old date when the edit moves that date.
 */
export function otherVersions(yamls: readonly string[], validFrom: string | null): string[] {
  if (!validFrom) return [...yamls];
  return yamls.filter((yaml) => versionValidFrom(yaml) !== validFrom);
}
