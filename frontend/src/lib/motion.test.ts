import { afterEach, describe, expect, it, vi } from 'vitest';
import { scrollBehavior } from './motion';

function prefersReducedMotion(reduce: boolean) {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: reduce && query === '(prefers-reduced-motion: reduce)',
  }));
}

describe('scrollBehavior', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('scrolls smoothly by default', () => {
    prefersReducedMotion(false);
    expect(scrollBehavior()).toBe('smooth');
  });

  it('jumps when the viewer asked for less motion', () => {
    prefersReducedMotion(true);
    expect(scrollBehavior()).toBe('auto');
  });
});
