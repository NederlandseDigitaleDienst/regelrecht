import { afterEach, describe, expect, it, vi } from 'vitest';
import { todayLocalIso } from './localDate';

// A moment as a viewer in Amsterdam sees it at 00:30 on 1 January: the local
// fields say 2027-01-01, the instant is still 31 December in UTC. The getters
// stand in for the time zone, because a test worker cannot change the
// process's zone, and in UTC (CI's default) the two dates never differ.
class JustAfterMidnightInAmsterdam extends Date {
  constructor() {
    super('2026-12-31T23:30:00Z');
  }
  getFullYear() {
    return 2027;
  }
  getMonth() {
    return 0;
  }
  getDate() {
    return 1;
  }
}

describe('todayLocalIso', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('gives the local calendar date, not the UTC one', () => {
    expect(todayLocalIso(new JustAfterMidnightInAmsterdam())).toBe('2027-01-01');
  });

  it('pads month and day', () => {
    expect(todayLocalIso(new Date(2026, 2, 5, 12))).toBe('2026-03-05');
  });

  it('defaults to now', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 10, 11, 23, 59));
    expect(todayLocalIso()).toBe('2026-11-11');
  });
});
