import { describe, it, expect } from 'vitest';
import { useLatest } from './useLatest';

describe('useLatest', () => {
  it('the only claim is current', () => {
    const claim = useLatest();
    const isCurrent = claim();
    expect(isCurrent()).toBe(true);
  });

  it('a newer claim invalidates older ones', () => {
    const claim = useLatest();
    const first = claim();
    const second = claim();
    expect(first()).toBe(false);
    expect(second()).toBe(true);
    const third = claim();
    expect(second()).toBe(false);
    expect(third()).toBe(true);
  });

  it('independent instances do not interfere', () => {
    const claimA = useLatest();
    const claimB = useLatest();
    const a = claimA();
    claimB();
    expect(a()).toBe(true);
  });

  it('guards the stale-async-response race', async () => {
    const claim = useLatest();
    const writes: string[] = [];

    async function load(value: string, delay: number) {
      const isCurrent = claim();
      await new Promise((r) => setTimeout(r, delay));
      if (!isCurrent()) return;
      writes.push(value);
    }

    // Slow "old" load resolves after the fast "new" one - only the
    // newest may write.
    await Promise.all([load('old', 20), load('new', 0)]);
    expect(writes).toEqual(['new']);
  });
});
