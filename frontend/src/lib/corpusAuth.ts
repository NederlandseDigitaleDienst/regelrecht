/**
 * Mirror of the backend's token derivation, for display only.
 *
 * The editor-api derives an auth ref from the repo coordinates
 * (`derive_auth_ref` in packages/editor-api/src/trajects.rs) and turns that
 * into an env-var name (`token_env_name` in packages/corpus/src/auth.rs). The
 * traject form shows operators the exact name they have to set, so it has to
 * arrive at the same string.
 *
 * Kept in two steps like the backend, and pinned by corpusAuth.test.ts with the
 * fixtures from the Rust unit tests: a change on either side then shows up as a
 * failing test instead of a wrong name on screen.
 */

/** Lowercase `owner/repo`, every run of non-alphanumerics collapsed to one dash. */
export function deriveAuthRef(owner: string, repo: string): string {
  return `${owner}/${repo}`
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

/** The env var the backend looks up for an auth ref. */
export function tokenEnvName(authRef: string): string {
  return `CORPUS_AUTH_${authRef.toUpperCase().replace(/-/g, '_')}_TOKEN`;
}
