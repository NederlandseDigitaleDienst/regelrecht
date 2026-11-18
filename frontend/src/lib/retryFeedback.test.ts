import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { holdRetryFloor, sleep, RETRY_MIN_SPINNER_MS } from './retryFeedback';

describe('retryFeedback', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  /** Resolved-or-not probe: flips `done` once the promise settles. */
  function track(promise: Promise<unknown>) {
    const state = { done: false };
    promise.then(() => {
      state.done = true;
    });
    return state;
  }

  it('sleep resolves immediately for non-positive durations', async () => {
    const zero = track(sleep(0));
    const negative = track(sleep(-100));
    await Promise.resolve();
    expect(zero.done).toBe(true);
    expect(negative.done).toBe(true);
  });

  it('holdRetryFloor is a no-op when the load did not fail', async () => {
    const held = track(holdRetryFloor({ startedAt: 0, minMs: RETRY_MIN_SPINNER_MS, failed: false }));
    await Promise.resolve();
    expect(held.done).toBe(true);
  });

  it('holdRetryFloor is a no-op when no floor is set', async () => {
    const held = track(holdRetryFloor({ startedAt: 0, minMs: 0, failed: true }));
    await Promise.resolve();
    expect(held.done).toBe(true);
  });

  it('holds a fast failure for the remainder of the floor', async () => {
    // Load started at t=0 and failed at t=300ms.
    vi.setSystemTime(300);
    const held = track(holdRetryFloor({ startedAt: 0, minMs: 2000, failed: true }));

    // Not yet: 1700ms still to go.
    await vi.advanceTimersByTimeAsync(1699);
    expect(held.done).toBe(false);

    // The floor elapses exactly at the 2000ms mark.
    await vi.advanceTimersByTimeAsync(1);
    expect(held.done).toBe(true);
  });

  it('does not wait when the load already outlasted the floor', async () => {
    // Failed at t=2500ms — past the 2000ms floor, so nothing to hold.
    vi.setSystemTime(2500);
    const held = track(holdRetryFloor({ startedAt: 0, minMs: 2000, failed: true }));
    await Promise.resolve();
    expect(held.done).toBe(true);
  });
});
