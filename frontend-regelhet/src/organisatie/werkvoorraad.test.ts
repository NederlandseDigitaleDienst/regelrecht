// One work queue over the processes of an organisation: the rows of each
// process made as its own back office makes them, the longest waiting first,
// a process that failed named apart while the others stay, a filter per
// process, and a row that opens the handling screen through the hand-off.
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Casus, Gram, Kroniek, Stand, Vormen } from '../gedeeld/types';
import type { Proces } from './api';
import type { ProcesAntwoord } from './perProces';
import { ALLE, behandelAdres, filter, samenvoegen, vraagWerkvoorraad } from './werkvoorraad';
import type { ProcesWerkvoorraad } from './werkvoorraad';

afterEach(() => {
  vi.unstubAllGlobals();
});

const proces = (id: string, titel: string): Proces => ({ id, naam: id, titel, bereikbaar: true });
const parkeerkaart = proces('tnt-p/parkeerkaart', 'Gehandicaptenparkeerkaart');
const bedrijven = proces('tnt-b/bedrijfsparkeren', 'Bedrijfsparkeervergunning');
const ander = proces('tnt-x/ander', 'Andere regeling');

const casus = (cel: string, titel: string): Casus => ({
  id: cel,
  cel,
  cel_naam: `gemeente ${cel}`,
  artikel: 'wet#1',
  besluit: 'wet#1',
  andere_cellen: [],
  indieningen: [{ artikel: 'wet#1', besluit: 'wet#1', cel, cel_naam: `gemeente ${cel}`, soort: 'AANVRAAG' }],
  titel,
  vandaag: '2026-11-25',
});
const stuk = (id: string, op: string): Gram => ({ id, kind: 'AANVRAAG', basis: 'wet#1', recorded_on: op, recording_actor: 'gemeente' });

const antwoord = (p: Proces, cel: string, stukken: Gram[]): ProcesAntwoord<ProcesWerkvoorraad> => ({
  proces: p,
  antwoord: {
    casus: casus(cel, p.titel ?? p.naam),
    vormen: { indieningen: {}, besluiten: {} } as unknown as Vormen,
    verzoeken: [],
    werkvoorraad: { cel, cel_naam: `gemeente ${cel}`, open: stukken.map((s) => ({ stuk: s })) },
  },
});

describe('the work queue over every process', () => {
  const antwoorden = [
    antwoord(parkeerkaart, 'wilgendaal', [stuk('p1', '2026-11-20'), stuk('p2', '2026-11-23')]),
    { proces: ander, fout: 'Andere regeling is not reachable' },
    antwoord(bedrijven, 'wilgendaal', [stuk('b1', '2026-11-21')]),
  ];

  it('puts the rows of every process that answered in one list, the longest waiting first', () => {
    const { rijen } = samenvoegen(antwoorden);
    expect(rijen.map((r) => `${r.proces.id}:${r.stuk.id}`)).toEqual([
      'tnt-p/parkeerkaart:p1',
      'tnt-b/bedrijfsparkeren:b1',
      'tnt-p/parkeerkaart:p2',
    ]);
    expect(rijen[0]?.regel).toMatch(/^Ontvangen /);
    expect(rijen[0]?.cel).toBe('wilgendaal');
  });

  it('names a process that failed apart, and keeps the rows of the others', () => {
    const { rijen, onbereikbaar } = samenvoegen(antwoorden);
    expect(onbereikbaar).toEqual([{ proces: ander, fout: 'Andere regeling is not reachable' }]);
    expect(rijen).toHaveLength(3);
  });

  it('filters on one process, or shows all', () => {
    const { rijen } = samenvoegen(antwoorden);
    expect(filter(rijen, bedrijven.id).map((r) => r.stuk.id)).toEqual(['b1']);
    expect(filter(rijen, ALLE)).toHaveLength(3);
    expect(filter(rijen, ander.id)).toEqual([]);
  });

  it('orders rows of one day by the id of the submission, and a revoked one by the record to decide after', () => {
    const naGram = stuk('n0', '2026-11-22');
    const kroniek: Kroniek = { cel: 'wilgendaal', cel_naam: 'gemeente wilgendaal', kop: 1, grammen: [naGram], stand: { rollen: {}, herroepen: {}, antwoorden: {} } };
    const herroepen: ProcesAntwoord<ProcesWerkvoorraad> = {
      proces: parkeerkaart,
      antwoord: {
        casus: casus('wilgendaal', 'Gehandicaptenparkeerkaart'),
        vormen: { indieningen: {}, besluiten: {} } as unknown as Vormen,
        verzoeken: [],
        werkvoorraad: { cel: 'wilgendaal', cel_naam: 'gemeente wilgendaal', open: [{ stuk: stuk('p0', '2026-11-01'), na: 'n0' }, { stuk: stuk('p9', '2026-11-21') }] },
        kroniek,
      },
    };
    const zelfdeDag = antwoord(bedrijven, 'wilgendaal', [stuk('b1', '2026-11-21')]);
    const { rijen } = samenvoegen([herroepen, zelfdeDag]);
    // b1 and p9 came in on one day: b1's id sorts first; p0 waits since the 22nd.
    expect(rijen.map((r) => r.stuk.id)).toEqual(['b1', 'p9', 'p0']);
    const opnieuw = rijen[2];
    expect(opnieuw?.opnieuw).toBe(true);
    expect(opnieuw?.sinds).toBe('2026-11-22');
    expect(opnieuw?.regel).toContain('Opnieuw beslissen na');
  });

  it('says whom a submission waits on, from the requests of its process', () => {
    const met: ProcesAntwoord<ProcesWerkvoorraad> = antwoord(parkeerkaart, 'wilgendaal', [stuk('p1', '2026-11-20')]);
    if ('antwoord' in met) {
      met.antwoord.verzoeken = [
        { verzoek: { ...stuk('v1', '2026-11-20'), fields: { aanvraag: 'p1' } }, aan: 'ggd', aan_naam: 'GGD Wilgendaal', open: true, antwoord_ontvangen: null },
      ];
    }
    expect(samenvoegen([met]).rijen[0]?.regel).toContain('Wacht op GGD Wilgendaal');
  });

  it("opens a row in its process's handling screen through the hand-off", () => {
    const rij = samenvoegen(antwoorden).rijen[0];
    expect(rij && behandelAdres(rij)).toBe(
      '/api/processen/tnt-p%2Fparkeerkaart/openen?naar=%2Fbackoffice%2Fbehandelen%2Fwilgendaal%2Fp1',
    );
  });
});

describe('asking one process for its queue', () => {
  function stub(werkvoorraad: unknown, kroniekStatus = 200) {
    const kroniek: Kroniek = { cel: 'wilgendaal', cel_naam: 'gemeente wilgendaal', kop: 0, grammen: [], stand: {} as Stand };
    const fetch = vi.fn(async (pad: string) => {
      if (pad.includes('/kroniek') && kroniekStatus !== 200) {
        return new Response(JSON.stringify({ fout: 'the chronicle failed' }), { status: kroniekStatus, headers: { 'content-type': 'application/json' } });
      }
      if (pad.endsWith('/backoffice/verzoeken')) {
        return new Response(JSON.stringify({ verzoeken: [] }), { status: 200, headers: { 'content-type': 'application/json' } });
      }
      const body = pad.endsWith('/api/casus')
        ? casus('wilgendaal', 'Gehandicaptenparkeerkaart')
        : pad.endsWith('/api/formulier')
          ? { indieningen: {}, besluiten: {} }
          : pad.includes('/kroniek')
            ? kroniek
            : werkvoorraad;
      return new Response(JSON.stringify(body), { status: 200, headers: { 'content-type': 'application/json' } });
    });
    vi.stubGlobal('fetch', fetch);
    return fetch;
  }
  const gevraagd = (fetch: ReturnType<typeof stub>) => fetch.mock.calls.map((c) => String((c as unknown[])[0])).sort();

  it('asks the case, the forms and the queue, and the chronicle only when a decision was revoked', async () => {
    let fetch = stub({ cel: 'wilgendaal', cel_naam: 'gemeente wilgendaal', open: [{ stuk: stuk('p1', '2026-11-20') }] });
    const zonder = await vraagWerkvoorraad(parkeerkaart);
    expect(zonder.kroniek).toBeUndefined();
    expect(gevraagd(fetch)).toEqual([
      '/api/processen/tnt-p%2Fparkeerkaart/api/backoffice/verzoeken',
      '/api/processen/tnt-p%2Fparkeerkaart/api/backoffice/werkvoorraad',
      '/api/processen/tnt-p%2Fparkeerkaart/api/casus',
      '/api/processen/tnt-p%2Fparkeerkaart/api/formulier',
    ]);
    fetch = stub({ cel: 'wilgendaal', cel_naam: 'gemeente wilgendaal', open: [{ stuk: stuk('p1', '2026-11-20'), na: 'x' }] });
    const met = await vraagWerkvoorraad(parkeerkaart);
    expect(met.kroniek?.cel).toBe('wilgendaal');
    expect(gevraagd(fetch)).toContain('/api/processen/tnt-p%2Fparkeerkaart/api/backoffice/cellen/wilgendaal/kroniek');
  });

  it('keeps the rows of a process whose chronicle fails, marked to decide again', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    stub({ cel: 'wilgendaal', cel_naam: 'gemeente wilgendaal', open: [{ stuk: stuk('p1', '2026-11-20'), na: 'x' }] }, 502);
    const zonder = await vraagWerkvoorraad(parkeerkaart);
    expect(zonder.kroniek).toBeUndefined();
    const rij = samenvoegen([{ proces: parkeerkaart, antwoord: zonder }]).rijen[0];
    expect(rij?.opnieuw).toBe(true);
    expect(rij?.regel).not.toContain('Opnieuw beslissen na');
  });
});
