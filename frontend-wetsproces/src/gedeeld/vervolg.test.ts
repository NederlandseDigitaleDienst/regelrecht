// How far the acts after a grant are, in words: an act the system took
// itself says so, for the employee and beside its row.
import { describe, expect, it } from 'vitest';
import type { NaToekenning, Vervolgstand } from './types';
import { burgerStand, doorHetSysteem, medewerkerStand } from './vervolg';

const na: NaToekenning = {
  stand: 'Uw vergunning is verleend.',
  handelingen: [
    { feit: 'beleid#7#vergunning_gedrukt', stand: 'Uw vergunning wordt gedrukt.', door_het_systeem: { artikel: 'beleid#7' } },
    { feit: 'beleid#5#vergunning_verzonden', knop: 'Verzenden', stand: 'Verzonden op {datum}.' },
  ],
};
const stand = (gedrukt: string | null, verzonden: string | null): Vervolgstand => ({
  besluit: 'b',
  toekenning: 'b',
  stappen: [
    { feit: 'beleid#7#vergunning_gedrukt', gedaan: gedrukt },
    { feit: 'beleid#5#vergunning_verzonden', gedaan: verzonden },
  ],
  volgende: null,
  toegekend_op: '2026-10-20',
});

describe('how far the acts after a grant are', () => {
  it('says of an act the system took that it did so automatically', () => {
    expect(medewerkerStand(stand('2026-10-20', null), na)).toBe('Vergunning gedrukt op 20 oktober 2026, automatisch');
    expect(medewerkerStand(stand('2026-10-20', '2026-11-04'), na)).toBe('Vergunning verzonden op 4 november 2026');
    expect(medewerkerStand(stand(null, null), na)).toBe('Toegekend op 20 oktober 2026');
  });

  it('gives the citizen the text of the last act done, with its day', () => {
    expect(burgerStand(stand(null, null), na)).toBe('Uw vergunning is verleend.');
    expect(burgerStand(stand('2026-10-20', '2026-11-04'), na)).toBe('Verzonden op 4 november 2026.');
  });

  it('marks a record the system made, by its article, and no other', () => {
    expect(doorHetSysteem('x', { x: 'wet#7' })).toContain('Automatisch vastgelegd volgens');
    expect(doorHetSysteem('y', { x: 'wet#7' })).toBe('');
    expect(doorHetSysteem('y')).toBe('');
  });
});
