// The prediction panel on the application form: an answer computed here that
// differs from the signed one shows as an error with both values, and a
// prediction that hangs on the GGD's advice shows one line per advice. The
// kern and the server stand in (voorspelling.ts and app.ts mocked); the
// nldd-* elements are not defined here, so the test reads their attributes.
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Indieningsvorm } from '../../gedeeld/types';
import type { Vergelijking, Voorspelling } from './rekenen';
import VoorspellingPaneel from './VoorspellingPaneel.vue';

const mocks = vi.hoisted(() => ({
  bereidVoor: vi.fn(),
  voorspel: vi.fn(),
  sluit: vi.fn(),
}));

vi.mock('./app', async () => {
  const { ref } = await import('vue');
  return { gekozen: ref('parkeerkaart'), burgerapp: async () => ({}), rekenkern: async () => () => ({}) };
});
vi.mock('./voorspelling', () => ({ bereidVoor: mocks.bereidVoor, voorspel: mocks.voorspel }));

const vorm: Indieningsvorm = { soort: 'AANVRAAG', basis: 'babw#49', velden: [], besluit: 'babw#49', cel: 'gemeente_wilgendaal' };

function voorbereid(antwoorden: Vergelijking[], regelwerkenGelijk = true) {
  return {
    gegevens: { iss: '00000099000000000001', peildatum: '2026-10-13' },
    kern: { sluit: mocks.sluit },
    vergelijking: { antwoorden, regelwerkenGelijk, leeg: false },
  };
}

async function teken() {
  const w = mount(VoorspellingPaneel, { props: { artikel: 'babw#49', vorm, velden: { kaart_als_bestuurder: true }, naam: 'Test Burger' } });
  await flushPromises();
  return w;
}

const dialoog = (w: Awaited<ReturnType<typeof teken>>, tekst: RegExp) =>
  w.findAll('nldd-inline-dialog').find((d) => tekst.test(d.attributes('text') ?? ''));

beforeEach(() => vi.clearAllMocks());

describe('the prediction panel', () => {
  it('shows each signed answer as matching when the device computes the same', async () => {
    mocks.bereidVoor.mockResolvedValue(
      voorbereid([{ naam: 'aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt', ondertekend: true, lokaal: true, gelijk: true }]),
    );
    const w = await teken();
    expect(dialoog(w, /iets anders/)).toBeUndefined();
    const rij = w.find('nldd-list-item');
    expect(rij.find('nldd-text-cell').attributes('text')).toBe('Aanvrager eerder gehandicaptenparkeerkaart verstrekt');
    expect(rij.find('nldd-tag').attributes()).toMatchObject({ text: 'Klopt', color: 'success' });
    expect(w.text()).toContain('00000099000000000001');
  });

  it('shows a silent cell as unknown, not as no', async () => {
    mocks.bereidVoor.mockResolvedValue(
      voorbereid([{ naam: 'aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt', ondertekend: null, lokaal: null, gelijk: true }]),
    );
    const w = await teken();
    expect(w.find('nldd-list-item nldd-text-cell').attributes('supporting-text')).toBe('Niet bekend bij de regeling');
  });

  it('shows a mismatch clearly, with what was signed and what the device computed', async () => {
    mocks.bereidVoor.mockResolvedValue(
      voorbereid([{ naam: 'advies_langdurige_loopbeperking', ondertekend: null, lokaal: false, gelijk: false }]),
    );
    const w = await teken();
    expect(dialoog(w, /iets anders/)?.attributes('icon-color')).toBe('critical');
    const rij = w.find('nldd-list-item');
    expect(rij.find('nldd-tag').attributes()).toMatchObject({ text: 'Verschil', color: 'critical' });
    expect(rij.find('nldd-text-cell').attributes('supporting-text')).toBe('Ondertekend: Niet bekend bij de regeling. Op dit apparaat: Nee.');
  });

  it('says so when the device has other versions of the regelwerken than the executor computed on', async () => {
    mocks.bereidVoor.mockResolvedValue(voorbereid([], false));
    const w = await teken();
    expect(dialoog(w, /andere versies/)?.attributes('icon-color')).toBe('critical');
  });

  it('predicts on the form’s values, and shows a conditional outcome per advice of the GGD', async () => {
    mocks.bereidVoor.mockResolvedValue(voorbereid([]));
    const v: Voorspelling = {
      nu: { uitkomst: 'afgewezen' },
      zeker: false,
      takken: [
        { celNaam: 'GGD Wilgendaal', taak: 'Geneeskundig advies uitbrengen', vastgesteld: ['langdurige loopbeperking'], uitkomst: 'toegekend' },
        { celNaam: 'GGD Wilgendaal', taak: 'Geneeskundig advies uitbrengen', vastgesteld: [], anders: true, uitkomst: 'afgewezen' },
      ],
    };
    mocks.voorspel.mockReturnValue(v);
    const w = await teken();
    await w.find('nldd-button[text="Voorspel de uitkomst"]').trigger('click');
    expect(mocks.voorspel).toHaveBeenCalledWith(expect.anything(), vorm, 'babw#49', { kaart_als_bestuurder: true }, 'Test Burger');
    const rijen = w.findAll('nldd-list[accessible-label="Mogelijke uitkomsten"] nldd-list-item');
    expect(rijen.map((r) => [r.find('nldd-text-cell').attributes('text'), r.find('nldd-tag').attributes('text')])).toEqual([
      ['Als GGD Wilgendaal vaststelt: langdurige loopbeperking', 'Toegekend'],
      ['In alle andere gevallen', 'Afgewezen'],
    ]);
    // Changing the form takes the prediction away: it was about other answers.
    await w.setProps({ velden: { kaart_als_bestuurder: false } });
    expect(w.findAll('nldd-list[accessible-label="Mogelijke uitkomsten"]')).toHaveLength(0);
  });

  it('says so when the procedure hands out no data yet', async () => {
    mocks.bereidVoor.mockResolvedValue({ ...voorbereid([]), vergelijking: { antwoorden: [], regelwerkenGelijk: true, leeg: true } });
    const w = await teken();
    expect(w.text()).toContain('Deze procedure geeft nog geen gegevens uit.');
  });

  it('shows one outcome when nothing that comes after the application changes it', async () => {
    mocks.bereidVoor.mockResolvedValue(voorbereid([]));
    mocks.voorspel.mockReturnValue({
      nu: { uitkomst: 'toegekend' },
      zeker: true,
      takken: [{ celNaam: 'GGD Wilgendaal', taak: 't', vastgesteld: [], uitkomst: 'toegekend' }],
    } satisfies Voorspelling);
    const w = await teken();
    await w.find('nldd-button[text="Voorspel de uitkomst"]').trigger('click');
    expect(dialoog(w, /Verwachte uitkomst/)?.attributes()).toMatchObject({ text: 'Verwachte uitkomst: toegekend', variant: 'success' });
  });

  it('reads a refusal of the RegelHet as the core gives it', async () => {
    mocks.bereidVoor.mockRejectedValue({ kind: 'service', status: 404, body: { fout: 'deze regeling geeft geen gegevens uit' } });
    const w = await teken();
    expect(dialoog(w, /Voorspellen lukt niet/)?.attributes('supporting-text')).toBe('deze regeling geeft geen gegevens uit');
  });

  it('says why when the data do not load, and frees the kern when it closes', async () => {
    mocks.bereidVoor.mockRejectedValue(new Error('de handtekening onder de gegevens klopt niet met de sleutel van de organisatie'));
    const w = await teken();
    expect(dialoog(w, /Voorspellen lukt niet/)?.attributes('supporting-text')).toMatch(/handtekening/);
    w.unmount();
    mocks.bereidVoor.mockResolvedValue(voorbereid([]));
    const w2 = await teken();
    w2.unmount();
    expect(mocks.sluit).toHaveBeenCalledOnce();
  });
});
