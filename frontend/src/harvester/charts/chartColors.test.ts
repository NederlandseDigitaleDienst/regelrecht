import { describe, it, expect } from 'vitest';
import { resolveChartColors, type ChartColors } from './chartColors';

// happy-dom has no real canvas 2D context and no NLDD stylesheet, so this
// exercises the fallback path: every token resolves to its computed-style
// string (empty here) instead of a canvas-normalized rgb() value. The
// invariants that hold on both paths: every slot present as a string, and the
// probe cleaned up.
describe('resolveChartColors', () => {
  it('returns a string for every chart slot', () => {
    const colors = resolveChartColors();
    expect(colors).not.toBeNull();
    const keys: (keyof ChartColors)[] = [
      'succeeded',
      'failed',
      'added',
      'text',
      'textSecondary',
      'grid',
      'surface',
    ];
    for (const key of keys) {
      expect(colors?.[key], key).toBeTypeOf('string');
    }
  });

  it('removes the probe element again', () => {
    const before = document.body.childElementCount;
    resolveChartColors();
    expect(document.body.childElementCount).toBe(before);
  });
});
