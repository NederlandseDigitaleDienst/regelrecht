import { describe, expect, it } from 'vitest';
import { lijstVan, lijstenVanIndiening, lijstenVanToekenning, nieuwsteEerst, oudsteEerst } from './lijsten';

describe('the lists of the work queue', () => {
  it('opens Te doen unless the address names another list', () => {
    expect(lijstVan(undefined)).toBe('te-doen');
    expect(lijstVan('onbekend')).toBe('te-doen');
    expect(lijstVan('wacht')).toBe('wacht');
  });

  it('keeps a submission that waits on an answer out of Te doen, and every submission in Alle aanvragen', () => {
    expect(lijstenVanIndiening(true, false)).toEqual(['te-doen', 'alle']);
    expect(lijstenVanIndiening(true, true)).toEqual(['wacht', 'alle']);
    expect(lijstenVanIndiening(false, false)).toEqual(['alle']);
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

  it('puts what came in last first, and equal days in a fixed order', () => {
    const rijen = [
      { sleutel: 'b', ontvangen: '2026-10-13' },
      { sleutel: 'c', ontvangen: '2026-10-14' },
      { sleutel: 'a', ontvangen: '2026-10-13' },
    ];
    expect(nieuwsteEerst(rijen).map((r) => r.sleutel)).toEqual(['c', 'a', 'b']);
  });
});
