import { beforeEach, describe, expect, it, vi } from 'vitest';

const voorBeslissen = vi.fn(async () => ({ bevindingen: [] }));
const beslissen = vi.fn(async () => ({ besluit: {}, in_de_plaats: {}, waarom: {} }));
const grammen = [];
vi.mock('../src/api.js', () => ({
  api: { voorBeslissen, beslissen, kroniek: async () => ({ grammen }) },
}));

// state.js first, as the app loads it: awb.js and beroep.js import each
// other through it.
await import('../src/state.js');
const { eerdereOordelen, eigenOordelen } = await import('../src/oordelen.js');
const { awb } = await import('../src/awb.js');

// The judgements the article of the decision on objection asks itself, as
// the form of that decision lists them (`vormen`).
const VELDEN = [
  { naam: 'indiener_is_belanghebbende', type: 'boolean' },
  { naam: 'indiener_niet_in_verzuim', type: 'boolean' },
];
const id = (staart) => `019a0000-0000-7000-8000-0000${staart}`;
const bestreden = { id: id('0000bbbb'), kind: 'BESCHIKKING', basis: 'wet#1' };
const bezwaar = { id: id('0000cccc'), kind: 'BEZWAAR', basis: 'awb#7:11', refers_to: bestreden.id };
// The decision on the objection the court annulled: it judged the objector
// no interested party.
const vernietigd = {
  id: id('0000dddd'),
  kind: 'BESCHIKKING',
  basis: 'awb#7:11',
  refers_to: bezwaar.id,
  fields: { indiener_is_belanghebbende: false, indiener_niet_in_verzuim: true },
};
const ontvangst = { id: id('00001111'), kind: 'EXECUTOGRAM', refers_to: vernietigd.id, recorded_on: '2027-01-11' };

beforeEach(() => {
  grammen.splice(0, grammen.length, bestreden, bezwaar, vernietigd, ontvangst);
  voorBeslissen.mockClear();
  beslissen.mockClear();
});

describe('the judgements the deciding article asks itself', () => {
  it('a switch left off says no', () => {
    expect(eigenOordelen(VELDEN, { indiener_is_belanghebbende: true })).toEqual({
      indiener_is_belanghebbende: true,
      indiener_niet_in_verzuim: false,
    });
  });

  it('a decision taken again starts from what the annulled one recorded', () => {
    expect(eerdereOordelen(vernietigd, VELDEN)).toEqual({
      indiener_is_belanghebbende: false,
      indiener_niet_in_verzuim: true,
    });
    expect(eerdereOordelen(undefined, VELDEN)).toEqual({});
  });

  it('deciding again after a judgment passes the judgements as given', async () => {
    // The form starts from the annulled decision; the authority now judges
    // the objector an interested party.
    const oordeel = eigenOordelen(VELDEN, {
      ...eerdereOordelen(vernietigd, VELDEN),
      indiener_is_belanghebbende: true,
    });
    await awb.heroverwegingNaUitspraak(ontvangst.id, oordeel);
    expect(voorBeslissen).toHaveBeenCalledWith(bezwaar.id, oordeel, '2027-01-11', ontvangst.id);
    await awb.opnieuwBeslissenOpBezwaar(ontvangst.id, oordeel, '2027-01-18');
    expect(beslissen).toHaveBeenCalledWith(bezwaar.id, oordeel, '2027-01-18', ontvangst.id);
  });
});
