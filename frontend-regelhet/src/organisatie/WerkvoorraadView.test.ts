// The work queue of an organisation's front on the screen: the rows of every
// process with the process as a tag and a link through the hand-off, a notice
// for a process that does not answer, and the filter per process. The nldd-*
// elements are not defined in this test, so it reads the attributes Vue sets.
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import WerkvoorraadView from './WerkvoorraadView.vue';

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const processen = [
  { id: 'tnt-p/parkeerkaart', naam: 'parkeerkaart', titel: 'Gehandicaptenparkeerkaart', bereikbaar: true },
  { id: 'tnt-b/bedrijfsparkeren', naam: 'bedrijfsparkeren', titel: 'Bedrijfsparkeervergunning', bereikbaar: false },
];
const casus = {
  id: 'p',
  cel: 'wilgendaal',
  cel_naam: 'gemeente Wilgendaal',
  artikel: 'wet#1',
  besluit: 'wet#1',
  andere_cellen: [],
  indieningen: [{ artikel: 'wet#1', besluit: 'wet#1', cel: 'wilgendaal', cel_naam: 'gemeente Wilgendaal', soort: 'AANVRAAG' }],
  titel: 'Gehandicaptenparkeerkaart',
  vandaag: '2026-11-25',
};
const werkvoorraad = {
  cel: 'wilgendaal',
  cel_naam: 'gemeente Wilgendaal',
  open: [{ stuk: { id: 'p1', kind: 'AANVRAAG', basis: 'wet#1', recorded_on: '2026-11-20', recording_actor: 'wilgendaal' } }],
};

function front() {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (pad: string) => {
      const json = (body: unknown, status = 200) =>
        new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });
      if (pad === '/api/organisatie') return json({ naam: 'Gemeente Wilgendaal', logo: null, kant: 'backoffice', processen });
      if (pad.includes('bedrijfsparkeren')) return json({ fout: 'bedrijfsparkeren is not reachable' }, 502);
      if (pad.endsWith('/api/casus')) return json(casus);
      if (pad.endsWith('/api/formulier')) return json({ indieningen: {}, besluiten: {} });
      if (pad.endsWith('/backoffice/verzoeken')) return json({ verzoeken: [] });
      return json(werkvoorraad);
    }),
  );
  vi.spyOn(console, 'error').mockImplementation(() => {});
}

async function toon() {
  const w = mount(WerkvoorraadView);
  await flushPromises();
  return w;
}

describe('the work queue of the organisation', () => {
  it('shows the rows of the processes that answered, each with its process and the hand-off', async () => {
    front();
    const w = await toon();
    const rijen = w.findAll('nldd-list-item');
    expect(rijen).toHaveLength(1);
    expect(rijen[0]?.find('nldd-tag').attributes('text')).toBe('Gehandicaptenparkeerkaart');
    expect(rijen[0]?.attributes('href')).toBe(
      '/api/processen/tnt-p%2Fparkeerkaart/openen?naar=%2Fbackoffice%2Fbehandelen%2Fwilgendaal%2Fp1',
    );
  });

  it('names a process that does not answer, above the rows of the others', async () => {
    front();
    const w = await toon();
    const melding = w.find('nldd-inline-dialog');
    expect(melding.attributes('text')).toBe('Bedrijfsparkeervergunning gaf geen werkvoorraad');
    expect(melding.attributes('supporting-text')).toContain('bedrijfsparkeren is not reachable');
  });

  it('narrows the list to one process with the segmented control', async () => {
    front();
    const w = await toon();
    const keuze = w.find('nldd-segmented-control');
    expect(w.findAll('nldd-segmented-control-item').map((i) => i.attributes('text'))).toEqual([
      'Alle',
      'Gehandicaptenparkeerkaart',
      'Bedrijfsparkeervergunning',
    ]);
    keuze.element.dispatchEvent(new CustomEvent('change', { detail: { value: 'tnt-b/bedrijfsparkeren' } }));
    await flushPromises();
    expect(w.findAll('nldd-list-item')).toHaveLength(0);
    expect(w.text()).toContain('Er wacht niets op een besluit.');
    keuze.element.dispatchEvent(new CustomEvent('change', { detail: { value: 'alle' } }));
    await flushPromises();
    expect(w.findAll('nldd-list-item')).toHaveLength(1);
  });
});
