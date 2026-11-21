// Where the citizen goes after getting in: back to the screen of a link that
// brought them to the login, never outside the citizen's screens.

/**
 * The screen to go to after getting in: `terug` when it is a citizen's
 * screen of this app, else the dossier. Only a path under `/burger/` passes,
 * so a link cannot send someone elsewhere after the login.
 */
export function terugNaar(terug: unknown): string {
  return typeof terug === 'string' && terug.startsWith('/burger/') ? terug : '/burger';
}
