// The decision in short, before it is taken and after: its outcome and the
// article it is taken under, never the letter. Once taken, the letter is
// sent with one button and can be read in a sheet on request.
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { artikelNaam } from '../gedeeld/keten';
import type { Berichtenspec, Casus, Gram, Vormen } from '../gedeeld/types';
import BesluitWeergave from './BesluitWeergave.vue';

const vastleggen = vi.fn(async () => ({}));
const { maakBericht } = vi.hoisted(() => ({ maakBericht: vi.fn(() => ({ schendingen: [] as unknown[] })) }));
vi.mock('./api', () => ({ backofficeApi: { vastleggen: (...a: unknown[]) => vastleggen(...(a as [])) } }));
vi.mock('../gedeeld/api', () => ({ gedeeldApi: { klok: async () => ({ datum: '2026-11-25' }) } }));
// What goes into the frozen message is bericht.test.ts's concern; here only that the button sends one.
vi.mock('../gedeeld/bericht', async (eigen) => ({
  ...(await eigen<typeof import('../gedeeld/bericht')>()),
  berichtsoortVoor: () => ({ id: 'beschikking' }),
  maakBericht,
  verzendvelden: () => ({ inhoud: 'brief', hash: 'h' }),
}));

enableAutoUnmount(afterEach);

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'wet#1',
  besluit: 'wet#2',
  uitkomst: 'verleend',
  andere_cellen: [],
  indieningen: [],
  vandaag: '2026-12-01',
};
const besluit: Gram = {
  id: 'b',
  kind: 'BESCHIKKING',
  basis: 'wet#2',
  refers_to: 'a',
  outputs: { verleend: false },
  recorded_on: '2026-11-25',
  recording_actor: 'gemeente',
};
const berichten = { berichtsoorten: [], verzending: { feit: 'beleid#9#bericht_verzonden' } } as unknown as Berichtenspec;
const vormen = { teksten: {}, indieningen: {}, besluiten: {}, berichten } as unknown as Vormen;

function toon(voorlopig: boolean) {
  return mount(BesluitWeergave, {
    props: { besluit, casus, vormen, celNaam: () => 'gemeente Voorbeeld', voorlopig, toegewezen: false, grammen: [besluit] },
    global: { stubs: { teleport: true, Bericht: { name: 'Bericht', template: '<div class="brief" />' } } },
  });
}
const knoppen = (w: ReturnType<typeof toon>) => w.findAll('nldd-button').map((b) => b.attributes('text'));

describe('the decision in short', () => {
  it('shows the outcome and the article before it is taken, and no letter or way to send one', () => {
    const w = toon(true);
    const tag = w.find('nldd-title nldd-tag');
    expect([tag.attributes('text'), tag.attributes('color')]).toEqual(['Afgewezen', 'critical']);
    const grondslag = w.findAll('nldd-list-item').map((r) => r.findAll('nldd-text-cell').map((c) => c.attributes('text'))).find((r) => r[0] === 'Op grond van');
    expect(grondslag).toEqual(['Op grond van', artikelNaam('wet#2')]);
    expect(w.find('.brief').exists()).toBe(false);
    expect(knoppen(w)).not.toContain('Bericht verzenden');
    expect(w.find('a[href="#brief"]').exists()).toBe(false);
  });

  it('once taken, sends the letter with one button without showing it', async () => {
    const w = toon(false);
    expect(w.find('.brief').exists()).toBe(false);
    // A letter that keeps its template has no banner.
    expect(w.find('nldd-banner[variant="critical"]').exists()).toBe(false);
    const verzend = w.findAll('nldd-button').find((b) => b.attributes('text') === 'Bericht verzenden');
    expect(verzend).toBeDefined();
    await verzend?.trigger('click');
    await flushPromises();
    expect(vastleggen).toHaveBeenCalledWith(
      { cel: 'gemeente', feit: 'beleid#9#bericht_verzonden', velden: { inhoud: 'brief', hash: 'h' } },
      expect.any(String),
    );
    expect(w.emitted('verzonden')).toHaveLength(1);
  });

  it('names a violation of the letter beside the send button, since the letter itself is not on the screen', () => {
    maakBericht.mockReturnValueOnce({ schendingen: [{ soort: 'deel', tekst: 'Ondertekening ontbreekt' }] });
    const w = toon(false);
    const banner = w.find('nldd-banner[variant="critical"]');
    expect(banner.attributes('text')).toBe('Het bericht schendt een invariant van zijn sjabloon');
  });

  it('shows the letter in a sheet only when asked', async () => {
    const w = toon(false);
    const sheet = () => w.find('nldd-sheet[accessible-label="Brief"]');
    expect(sheet().attributes('open')).toBeUndefined();
    await w.find('a[href="#brief"]').trigger('click');
    expect(sheet().attributes('open')).toBe('true');
    expect(sheet().find('.brief').exists()).toBe(true);
  });
});
