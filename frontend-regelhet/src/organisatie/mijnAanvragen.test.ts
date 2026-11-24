// "Mijn aanvragen" over the processes of an organisation: the rows of every
// process in one list, newest first, each opening its application in its own
// process; a process that fails is named apart and leaves the others; a
// process that knows citizens by another identifier is left out. Also the
// page that shows the list. The nldd-* elements are not defined in these
// tests, so they read the attributes Vue sets on them.
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Casus, Gram, Stand } from '../gedeeld/types';
import type { Proces } from './api';
import { mijnAanvragen } from './mijnAanvragen';
import MijnAanvragenView from './MijnAanvragenView.vue';

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const PARKEERKAART: Proces = { id: 'tnt-p/parkeerkaart', naam: 'parkeerkaart', titel: 'Gehandicaptenparkeerkaart', bereikbaar: true };
const BEDRIJF: Proces = { id: 'tnt-b/bedrijfsparkeren', naam: 'bedrijfsparkeren', titel: 'Bedrijfsparkeervergunning', bereikbaar: true };
const VERGUNNING: Proces = { id: 'tnt-v/vergunning', naam: 'vergunning', titel: 'Parkeervergunning', bereikbaar: true };
const IIT: Proces = { id: 'tnt-i/iit', naam: 'iit', titel: 'Individuele inkomenstoeslag', bereikbaar: false };

const casus = (titel: string): Casus => ({
  id: titel,
  titel,
  cel: 'gemeente',
  cel_naam: 'gemeente Wilgendaal',
  artikel: 'wet#1',
  besluit: 'wet#1',
  uitkomst: 'verleend',
  andere_cellen: [],
  indieningen: [{ artikel: 'wet#1', besluit: 'wet#1', cel: 'gemeente', cel_naam: 'gemeente Wilgendaal', soort: 'AANVRAAG' }],
  vandaag: '2026-11-24',
});
const gram = (id: string, kind: string, dag: string, refers_to?: string, outputs?: Record<string, unknown>): Gram => ({
  id,
  kind,
  basis: 'wet#1',
  refers_to,
  outputs,
  recorded_on: dag,
  recording_actor: 'gemeente',
});

/** A dossier as the server sends it, with `besluiten` the decision answering each application. */
function dossier(grammen: Gram[], besluiten: Record<string, string> = {}): { grammen: Gram[]; stand: Stand } {
  const rollen: Stand['rollen'] = Object.fromEntries(grammen.map((g) => [g.id, g.kind === 'AANVRAAG' ? 'indiening' : 'besluit']));
  const antwoorden: Stand['antwoorden'] = Object.fromEntries(
    grammen
      .filter((g) => g.kind === 'AANVRAAG')
      .map((g) => {
        const besluit = besluiten[g.id];
        return [g.id, besluit ? { soort: 'besloten' as const, besluit } : { soort: 'open' as const }];
      }),
  );
  return { grammen, stand: { rollen, herroepen: {}, antwoorden } };
}

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

/**
 * The front: the parking card has a decided application and an open one, the
 * parking permit one application dated between them, the
 * company permit refuses the dossier (it knows companies by KvK number), and
 * the income supplement does not answer.
 */
function front() {
  const fetch = vi.fn(async (input: RequestInfo | URL) => {
    const url = decodeURIComponent(String(input));
    if (url === '/api/organisatie') {
      return json({ naam: 'Gemeente Wilgendaal', logo: null, kant: 'burger', processen: [PARKEERKAART, BEDRIJF, IIT] });
    }
    if (url.startsWith(`/api/processen/${IIT.id}/`)) return json({ fout: 'iit is not reachable' }, 502);
    if (url === `/api/processen/${BEDRIJF.id}/api/dossier`) {
      return json({ fout: 'your account carries no identity to act as a citizen with' }, 403);
    }
    if (url === `/api/processen/${BEDRIJF.id}/api/casus`) return json(casus('Bedrijfsparkeervergunning'));
    if (url === `/api/processen/${VERGUNNING.id}/api/casus`) return json(casus('Parkeervergunning'));
    if (url === `/api/processen/${VERGUNNING.id}/api/dossier`) return json(dossier([gram('v1', 'AANVRAAG', '2026-11-01')]));
    if (url === `/api/processen/${PARKEERKAART.id}/api/casus`) return json(casus('Gehandicaptenparkeerkaart'));
    if (url === `/api/processen/${PARKEERKAART.id}/api/dossier`) {
      return json(
        dossier(
          [
            gram('a1', 'AANVRAAG', '2026-10-01'),
            gram('b1', 'BESCHIKKING', '2026-10-05', 'a1', { verleend: true }),
            gram('a2', 'AANVRAAG', '2026-11-20'),
          ],
          { a1: 'b1' },
        ),
      );
    }
    return json({ fout: 'not here' }, 404);
  });
  vi.stubGlobal('fetch', fetch);
  return fetch;
}

describe("the citizen's applications over the processes", () => {
  it('puts the rows of every process in one list, newest first, each opening its application in its process', async () => {
    front();
    const lijst = await mijnAanvragen([PARKEERKAART, BEDRIJF, VERGUNNING, IIT]);
    expect(lijst.aanvragen.map((a) => [a.id, a.proces.naam, a.datum, a.stand?.tekst])).toEqual([
      ['a2', 'parkeerkaart', '2026-11-20', 'In behandeling'],
      ['v1', 'vergunning', '2026-11-01', 'In behandeling'],
      ['a1', 'parkeerkaart', '2026-10-01', 'Toegewezen'],
    ]);
    expect(lijst.aanvragen[0]?.tekst).toBe('Aanvraag gehandicaptenparkeerkaart');
    expect(lijst.aanvragen[0]?.href).toBe('/api/processen/tnt-p%2Fparkeerkaart/openen?naar=%2Fburger%2Fdossier%2Fa2');
  });

  it('names a process that failed apart, and leaves out one that knows citizens by another identifier', async () => {
    front();
    const lijst = await mijnAanvragen([PARKEERKAART, BEDRIJF, IIT]);
    expect(lijst.nietBereikbaar.map((p) => p.naam)).toEqual(['iit']);
    expect(lijst.aanvragen.some((a) => a.proces.naam === 'bedrijfsparkeren')).toBe(false);
  });

  it('asks the processes only through the front', async () => {
    const fetch = front();
    await mijnAanvragen([PARKEERKAART, BEDRIJF, IIT]);
    for (const [url] of fetch.mock.calls) expect(String(url)).toMatch(/^\/api\/processen\/[^/]+\/api\/(casus|dossier)$/);
  });
});

describe('the page "Mijn aanvragen"', () => {
  async function toon() {
    const w = mount(MijnAanvragenView);
    await flushPromises();
    return w;
  }

  it('shows a row per application with its process, day and status, linking into the process', async () => {
    front();
    const w = await toon();
    const rijen = w.findAll('nldd-list-item');
    expect(rijen).toHaveLength(2);
    expect(rijen[0]?.find('nldd-text-cell').attributes('text')).toBe('Aanvraag gehandicaptenparkeerkaart');
    expect(rijen[0]?.find('nldd-text-cell').attributes('supporting-text')).toBe('20 november 2026');
    expect(rijen[0]?.find('nldd-tag').attributes('text')).toBe('Gehandicaptenparkeerkaart');
    expect(rijen[1]?.find('nldd-cell nldd-text').text()).toBe('Toegewezen');
    expect(rijen[1]?.attributes('href')).toBe('/api/processen/tnt-p%2Fparkeerkaart/openen?naar=%2Fburger%2Fdossier%2Fa1');
  });

  it('says which process did not answer, and keeps the rest', async () => {
    front();
    const w = await toon();
    const meldingen = w.findAll('nldd-inline-dialog');
    expect(meldingen.map((m) => m.attributes('text'))).toEqual(['Individuele inkomenstoeslag is nu niet bereikbaar']);
    expect(w.findAll('nldd-list-item')).toHaveLength(2);
  });

  it('says so when there is nothing yet', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo | URL) => {
        const url = decodeURIComponent(String(input));
        if (url === '/api/organisatie') return json({ naam: 'Gemeente Wilgendaal', logo: null, kant: 'burger', processen: [PARKEERKAART] });
        if (url.endsWith('/api/casus')) return json(casus('Gehandicaptenparkeerkaart'));
        return json(dossier([]));
      }),
    );
    const w = await toon();
    expect(w.find('nldd-inline-dialog').attributes('text')).toBe('U hebt nog niets aangevraagd');
    expect(w.find('nldd-list').exists()).toBe(false);
  });
});
