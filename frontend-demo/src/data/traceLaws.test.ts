import { describe, expect, it } from 'vitest';
import { traceLaws } from './traceLaws';

// Ingekort uit een echte trace van de zorgtoeslagwet (scenario "Persoon boven
// 18 heeft recht op zorgtoeslag"), met de vorm die trace.rs schrijft.
const TRACE = [
  'zorgtoeslagwet (2024-02-01 {bsn: 999993653} hoogte_toeslag)',
  '╟──Evaluating rules for zorgtoeslagwet (hoogte_toeslag)',
  '║   ╟──Reference: wet_brp#leeftijd',
  '║   ║   ╟──Resolving from PARAMETERS: $BSN = 999993653',
  '║   ║   └──Result: leeftijd = 19',
  '║   ╟──Reference: zvw#heeft_verzekering',
  '║   ║   ╟──Reference: wet_brp#land_van_verblijf',
  '║   ║   ║   └──Result: land_van_verblijf = NEDERLAND',
  '║   ║   ╟──Reference: penitentiaire_beginselenwet#is_gedetineerd',
  '║   ║   ║   └──Result: is_gedetineerd = false',
  '║   ║   └──Result: heeft_verzekering = true',
  '║   ├──Computing hoogte_toeslag',
  '║   │   └──Result: hoogte_toeslag = 2096',
  '╙──Result: hoogte_toeslag = 2096',
].join('\n');

describe('traceLaws', () => {
  const { lines, sections, laws } = traceLaws(TRACE);
  const lawOf = (i: number) => lines[i]?.law;

  it('geeft elke regel de wet waarbinnen hij draait', () => {
    expect(lawOf(0)).toBe('zorgtoeslagwet');
    expect(lawOf(3)).toBe('wet_brp');
    expect(lawOf(4)).toBe('wet_brp');
    expect(lawOf(6)).toBe('wet_brp');
    expect(lawOf(9)).toBe('penitentiaire_beginselenwet');
  });

  it('keert terug naar de buitenste wet als de boom terugkomt', () => {
    expect(lawOf(5)).toBe('zvw');
    expect(lawOf(10)).toBe('zvw');
    expect(lawOf(11)).toBe('zorgtoeslagwet');
    expect(lawOf(13)).toBe('zorgtoeslagwet');
  });

  it('markeert waar een andere wet het overneemt, maar niet dezelfde wet opnieuw', () => {
    // Regel 1 (`Evaluating rules for zorgtoeslagwet`) is geen nieuwe stop:
    // de kop erboven opende dezelfde wet al.
    expect(sections).toEqual([0, 2, 5, 6, 8]);
    expect(lines[1]?.start).toBe(false);
  });

  it('noemt elke wet één keer, in de volgorde waarin hij opduikt', () => {
    expect(laws).toEqual(['zorgtoeslagwet', 'wet_brp', 'zvw', 'penitentiaire_beginselenwet']);
  });

  it('opent geen wet voor een verwijzing naar iets dat geen wet is', () => {
    const text = [
      'zvw (2024-01-01 {bsn: 1} x)',
      '╟──Evaluating rules for zvw (x)',
      '║   ╟──Reference: untranslatable#art_3',
      '║   ║   └──Result: x = 1',
    ].join('\n');
    const r = traceLaws(text, (id) => id !== 'untranslatable');
    expect(r.laws).toEqual(['zvw']);
    expect(r.lines[3]?.law).toBe('zvw');
  });

  it('leest een wet-id met padsegmenten helemaal, niet tot de eerste slash', () => {
    const text = [
      'zorgtoeslagwet (2024-01-01 {bsn: 1} x)',
      '╟──Evaluating rules for zorgtoeslagwet (x)',
      '║   ╟──Reference: algemene_ouderdomswet/leeftijdsbepaling#pensioenleeftijd',
    ].join('\n');
    expect(traceLaws(text).laws).toEqual(['zorgtoeslagwet', 'algemene_ouderdomswet/leeftijdsbepaling']);
  });

  it('overleeft een lege trace', () => {
    expect(traceLaws('')).toEqual({ lines: [{ text: '', law: null, start: false }], sections: [], laws: [] });
    expect(traceLaws(undefined).laws).toEqual([]);
  });
});
