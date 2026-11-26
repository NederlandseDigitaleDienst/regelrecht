// Where someone goes after logging in: back to the screen of a link that
// brought them to the login, never outside the screens of this app. Each
// build says which screens count (`naar`), so the citizen's build takes no
// path of the back office.

/**
 * The screen to go to after logging in: `terug` when it is a path under
 * one of `naar` (such as `/burger/`), else `anders`. A link cannot send
 * anyone elsewhere after the login: another host, a protocol-relative `//`
 * or a path of another side all fall back.
 */
export function terugNaar(terug: unknown, naar: readonly string[], anders: string): string {
  return typeof terug === 'string' && naar.some((n) => terug.startsWith(n)) ? terug : anders;
}

/** The citizen's screens: what the citizen's build leads back to. */
export const BURGER = ['/burger/'] as const;

/** The screens of either side: what a build with the login leads back to. */
export const ALLE_KANTEN = ['/burger/', '/backoffice/'] as const;
