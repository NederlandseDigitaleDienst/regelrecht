import { describe, expect, it } from 'vitest';
import { lijstVan, lijstenVanIndiening, lijstenVanToekenning, oudsteEerst } from './lijsten';

describe('the lists of the work queue', () => {
  it('opens Te doen unless the address names another list', () => {
    expect(lijstVan(undefined)).toBe('te-doen');
    expect(lijstVan('onbekend')).toBe('te-doen');
    expect(lijstVan('wacht')).toBe('wacht');
  });

  it('keeps a submission that waits on an answer out of Te doen', () => {
    expect(lijstenVanIndiening(false)).toEqual(['te-doen', 'te-beslissen']);
    expect(lijstenVanIndiening(true)).toEqual(['te-beslissen', 'wacht']);
  });

  it('keeps a grant whose next act the system takes out of Te doen', () => {
    expect(lijstenVanToekenning(false)).toEqual(['te-doen', 'na-toekenning']);
    expect(lijstenVanToekenning(true)).toEqual(['na-toekenning', 'wacht']);
  });

  it('puts what waited longest first, and equal days in a fixed order', () => {
    const rijen = [
      { sleutel: 'c', sinds: '2026-10-14' },
      { sleutel: 'b', sinds: '2026-10-13' },
      { sleutel: 'a', sinds: '2026-10-13' },
    ];
    expect(oudsteEerst(rijen).map((r) => r.sleutel)).toEqual(['a', 'b', 'c']);
  });
});
