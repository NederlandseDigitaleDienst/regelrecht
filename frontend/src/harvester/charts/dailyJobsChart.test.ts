import { describe, it, expect } from 'vitest';
import {
  buildDailyJobsOption,
  formatAxisDate,
  SERIES_LABELS,
} from './dailyJobsChart';

const COLORS = {
  succeeded: '#0a0',
  failed: '#a00',
  added: '#00a',
  text: '#111',
  textSecondary: '#666',
  grid: '#eee',
  surface: '#fff',
};

const ENTRIES = [
  { date: '2026-07-06', added: 3, succeeded: 2, failed: 1 },
  { date: '2026-07-07', added: 0, succeeded: 0, failed: 0 },
];

describe('buildDailyJobsOption', () => {
  it('maps entries to stacked succeeded/failed bars and an added line', () => {
    const opt = buildDailyJobsOption(ENTRIES, COLORS);
    const [succeeded, failed, added] = opt.series;

    expect(succeeded.name).toBe(SERIES_LABELS.succeeded);
    expect(succeeded.type).toBe('bar');
    expect(failed.type).toBe('bar');
    // Same stack → the bars stack; added is a line, not stacked.
    expect(succeeded.stack).toBe(failed.stack);
    expect(added.type).toBe('line');
    expect(added).not.toHaveProperty('stack');

    expect(succeeded.data).toEqual([2, 0]);
    expect(failed.data).toEqual([1, 0]);
    expect(added.data).toEqual([3, 0]);
    expect(opt.xAxis.data).toHaveLength(2);
  });

  it('applies the injected colors per series', () => {
    const opt = buildDailyJobsOption(ENTRIES, COLORS);
    expect(opt.series.map((s) => s.color)).toEqual(['#0a0', '#a00', '#00a']);
  });

  it('starts the y-axis at zero with integer ticks', () => {
    const opt = buildDailyJobsOption(ENTRIES, COLORS);
    expect(opt.yAxis.type).toBe('value');
    expect(opt.yAxis.minInterval).toBe(1);
  });
});

describe('formatAxisNumber', () => {
  it('formats with nl-NL grouping', async () => {
    const { formatAxisNumber } = await import('./dailyJobsChart');
    expect(formatAxisNumber(1500)).toBe('1.500');
    expect(formatAxisNumber(0)).toBe('0');
  });
});

describe('formatAxisDate', () => {
  it('formats YYYY-MM-DD as a short Dutch date', () => {
    expect(formatAxisDate('2026-07-06')).toBe('6 jul');
  });

  it('passes through unparseable input', () => {
    expect(formatAxisDate('garbage')).toBe('garbage');
  });
});
