// Asking every process at once: each answer stays with its process, in order,
// and one that fails gives its reason without taking the others with it.
import { describe, expect, it } from 'vitest';
import type { Proces } from './api';
import { beantwoord, perProces, procesPad } from './perProces';

const proces = (id: string): Proces => ({ id, naam: id, titel: null, bereikbaar: true });

describe('asking every process', () => {
  it('keeps each answer with its process, and a failure with its reason', async () => {
    const processen = [proces('a/een'), proces('b/twee'), proces('c/drie')];
    const antwoorden = await perProces(processen, async (p) => {
      if (p.id === 'b/twee') throw new Error('b/twee is not reachable');
      return p.id.length;
    });
    expect(antwoorden.map((a) => a.proces.id)).toEqual(['a/een', 'b/twee', 'c/drie']);
    expect(antwoorden.filter(beantwoord).map((a) => a.antwoord)).toEqual([5, 6]);
    expect(antwoorden[1]).toEqual({ proces: processen[1], fout: 'b/twee is not reachable' });
  });

  it('names a route of a process through the front', () => {
    expect(procesPad('tnt-p/parkeerkaart', 'backoffice/werkvoorraad')).toBe('/api/processen/tnt-p%2Fparkeerkaart/api/backoffice/werkvoorraad');
  });
});
