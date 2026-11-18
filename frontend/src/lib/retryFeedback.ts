/**
 * retryFeedback — keep a retry's loading spinner on screen long enough to
 * register as feedback.
 *
 * A "Probeer opnieuw" button re-runs a fetch that flips a `loading` ref
 * true→false. When the retry fails again it can resolve within a frame, so
 * the spinner never paints and the error dialog snaps straight back — the
 * click feels dead, as if nothing happened. Holding the spinner for a short
 * floor on failure gives the user a visible "it tried again" beat.
 *
 * The floor applies ONLY on failure: a successful retry reveals the content,
 * which is its own feedback, so it should stay instant.
 */

/** Minimum time (ms) a failed retry keeps its spinner visible. */
export const RETRY_MIN_SPINNER_MS = 2000;

/** Resolve after `ms`; immediately when `ms <= 0`. */
export function sleep(ms: number): Promise<void> {
  return ms > 0 ? new Promise<void>((resolve) => setTimeout(resolve, ms)) : Promise.resolve();
}

/**
 * Await the remainder of the minimum-visible window for a load that just
 * finished. No-op unless `minMs` is set AND the load `failed` (and the
 * window hasn't already elapsed).
 *
 * @param {{ startedAt: number, minMs: number, failed: boolean }} args
 *   `startedAt` is a `Date.now()` stamp from when the load began.
 */
export function holdRetryFloor({
  startedAt,
  minMs,
  failed,
}: {
  startedAt: number;
  minMs: number;
  failed: boolean;
}): Promise<void> {
  if (!minMs || !failed) return Promise.resolve();
  return sleep(minMs - (Date.now() - startedAt));
}
