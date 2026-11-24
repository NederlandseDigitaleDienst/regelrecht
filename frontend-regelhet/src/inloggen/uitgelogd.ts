// That this tab just logged out, across the way out through Keycloak and
// back: the screen that logs in then says so instead of sending the browser
// to Keycloak again, which would log the person straight back in. Kept in
// the tab's session storage, read once. Without storage (a private window
// that blocks it) the screen goes on to Keycloak, which asks for the
// password after a logout: no loop either way.

const SLEUTEL = 'regelhet.uitgelogd';

/** Remember that this tab logs out now. */
export function markeerUitgelogd(): void {
  try {
    sessionStorage.setItem(SLEUTEL, '1');
  } catch {
    // No storage: see above.
  }
}

/** Whether this tab just logged out; asking forgets it. */
export function wasUitgelogd(): boolean {
  try {
    const was = sessionStorage.getItem(SLEUTEL) === '1';
    sessionStorage.removeItem(SLEUTEL);
    return was;
  } catch {
    return false;
  }
}
