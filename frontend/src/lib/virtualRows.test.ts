import { describe, it, expect } from 'vitest';
import { rowWindow, offsetOf } from './virtualRows';

const rows = (n: number, h = 50) => Array.from({ length: n }, () => h);

describe('rowWindow', () => {
  it('renders the rows in view plus the margin, and pads for the rest', () => {
    // 1000 rows of 50px; scrolled 5000px into the list, 500px view, 100px margin.
    const w = rowWindow(rows(1000), 5000, 500, 100, 20);
    expect(w.start).toBe(98); // 4900 / 50
    expect(w.end).toBe(112); // up to 5600
    expect(w.padStart).toBe(98 * 50);
    expect(w.padEnd).toBe((1000 - 112) * 50);
  });

  it('keeps the total height of the list, whatever is rendered', () => {
    const heights = rows(300).map((h, i) => h + (i % 3) * 20);
    const total = heights.reduce((a, b) => a + b, 0);
    for (const top of [0, 777, 5000, total]) {
      const w = rowWindow(heights, top, 400, 200, 20);
      const rendered = heights.slice(w.start, w.end).reduce((a, b) => a + b, 0);
      expect(w.padStart + rendered + w.padEnd).toBe(total);
    }
  });

  it('starts at the first row while the list starts below the top of the view', () => {
    const w = rowWindow(rows(1000), -300, 600, 100, 20);
    expect(w.start).toBe(0);
    expect(w.padStart).toBe(0);
    expect(w.end).toBe(8); // up to 400
  });

  it('reaches the last row at the bottom', () => {
    const w = rowWindow(rows(1000), 49_600, 400, 100, 20);
    expect(w.end).toBe(1000);
    expect(w.padEnd).toBe(0);
  });

  it('renders the first rows when there is no viewport yet', () => {
    const w = rowWindow(rows(1000), 0, 0, 100, 20);
    expect(w).toEqual({ start: 0, end: 20, padStart: 0, padEnd: 980 * 50 });
  });

  it('never renders an empty window for a list that has rows', () => {
    // Far below the list (more than viewport + margin) and past its end.
    const below = rowWindow(rows(100), -5000, 400, 100, 20);
    expect(below.end - below.start).toBeGreaterThan(0);
    const past = rowWindow(rows(100), 99_999, 400, 100, 20);
    expect(past).toEqual({ start: 99, end: 100, padStart: 99 * 50, padEnd: 0 });
  });

  it('handles an empty list', () => {
    expect(rowWindow([], 0, 500, 100, 20)).toEqual({ start: 0, end: 0, padStart: 0, padEnd: 0 });
  });
});

describe('offsetOf', () => {
  it('sums the heights before a row', () => {
    expect(offsetOf([10, 20, 30], 0)).toBe(0);
    expect(offsetOf([10, 20, 30], 2)).toBe(30);
    expect(offsetOf([10, 20, 30], 9)).toBe(60);
  });
});
