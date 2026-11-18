// Reload the page when a lazily loaded chunk no longer exists after a deploy.
//
// A poc is a static bundle: an open page keeps running the version it loaded,
// even after a new one is rolled out. That is intended. But the views load
// lazily (`() => import(...)` in router.js), and the chunk of a view this
// browser has not opened yet has a hashed name that the new image no longer
// serves. Without this handler, opening such a tab leaves the screen blank.
//
// The reload goes to the page the user was navigating to. What must survive
// (werkversie, variants, choices) lives in localStorage.
//
// At most one reload per ten seconds. If the chunk still fails after a reload,
// something else is wrong (server unreachable, mid-restart), and a reload loop
// would only hide that.

import type { RouteLocationNormalized } from 'vue-router';

/** The two router methods the handler uses. A vue-router `Router` fits. */
export interface StaleBundleRouter {
  onError(handler: (error: unknown, to?: RouteLocationNormalized) => unknown): unknown;
  resolve(to: RouteLocationNormalized): { href: string };
}

const STORAGE_KEY = 'regelrecht:stale-bundle-reload';
const MIN_INTERVAL_MS = 10_000;

// Chrome, Firefox and Safari each word a failed dynamic import differently.
const CHUNK_ERROR = /dynamically imported module|Importing a module script failed|Unable to preload CSS/i;

export function isChunkLoadError(error: unknown): boolean {
  return CHUNK_ERROR.test(String((error as { message?: unknown } | null | undefined)?.message ?? error ?? ''));
}

/** The part of `Storage` the loop guard uses, so tests can pass a stub. */
type GuardStorage = Pick<Storage, 'getItem' | 'setItem'>;

function readLast(storage: GuardStorage | null): number {
  try {
    return Number(storage?.getItem(STORAGE_KEY)) || 0;
  } catch {
    return 0;
  }
}

function writeLast(storage: GuardStorage | null, value: number): void {
  try {
    storage?.setItem(STORAGE_KEY, String(value));
  } catch {
    // No storage means no loop guard; reloading still works.
  }
}

function defaultStorage(): GuardStorage | null {
  try {
    return window.sessionStorage;
  } catch {
    return null;
  }
}

/**
 * Attach the handler to the router.
 *
 * Deliberately not to Vite's `vite:preloadError`: that event fires first and
 * only knows the current URL, so reloading there lands the user back on the
 * tab they were leaving. Letting the error reach the router reloads to the tab
 * they asked for. The pocs have no dynamic import outside the router that goes
 * through Vite's preload (the WASM imports are `@vite-ignore`).
 *
 * The options are for tests only.
 */
export function reloadOnStaleBundle(
  router: StaleBundleRouter,
  {
    target = window,
    storage = defaultStorage(),
    now = Date.now,
  }: {
    target?: { location: Pick<Location, 'assign' | 'reload'> };
    storage?: GuardStorage | null;
    now?: () => number;
  } = {},
): void {
  /** Reload to `href`, unless the loop guard blocks it. */
  function reload(href?: string) {
    const t = now();
    if (t - readLast(storage) < MIN_INTERVAL_MS) return;
    writeLast(storage, t);
    if (href) target.location.assign(href);
    else target.location.reload();
  }

  router.onError((error, to) => {
    if (!isChunkLoadError(error)) return;
    reload(to ? router.resolve(to).href : undefined);
  });
}
