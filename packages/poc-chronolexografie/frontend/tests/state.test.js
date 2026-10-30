import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { hervatten, hoofdvelden, state, stap, verversen } from '../src/state.js';

// The cell is not loaded here: the API hands back a fixed case and an empty
// chronicle, and the saved game is an opaque value.
const casus = { id: 'vakantieverlof', speeldatum: '2026-10-14', scenarios: [] };
vi.mock('../src/api.js', () => ({
  api: {
    kroniek: async () => ({ grammen: [], andere: [] }),
    vormen: async () => ({}),
    bewaren: async () => ({ spel: 'bewaard' }),
    herstellen: async () => casus,
  },
}));

// sessionStorage as the browser has it, for one tab.
const storage = new Map();
vi.stubGlobal('sessionStorage', {
  getItem: (k) => storage.get(k) ?? null,
  setItem: (k, v) => storage.set(k, String(v)),
  removeItem: (k) => storage.delete(k),
});

beforeEach(() => {
  storage.clear();
  Object.assign(state, { casus: null, scenario: null, refusals: [], grammen: [], andere: [], rechter: [], datum: '', fout: '' });
});

describe('saving the game of this tab', () => {
  it('restores the scenario and the refusals after a reload', async () => {
    Object.assign(state, { casus, datum: '2026-11-16', scenario: 'Tweede bezwaar', refusals: ['already replaces'] });
    await verversen();
    Object.assign(state, { casus: null, scenario: null, refusals: [], datum: '' });
    expect(await hervatten('vakantieverlof')).toBe('resumed');
    expect(state).toMatchObject({
      casus,
      datum: '2026-11-16',
      scenario: 'Tweede bezwaar',
      refusals: ['already replaces'],
    });
  });

  it('opens a save from before the guide in free play', async () => {
    storage.set('poc-spel', JSON.stringify({ casus: 'vakantieverlof', spel: {}, datum: '2026-10-20' }));
    Object.assign(state, { scenario: 'over', refusals: ['over'] });
    await hervatten('vakantieverlof');
    expect(state).toMatchObject({ casus, datum: '2026-10-20', scenario: null, refusals: [] });
  });

  it('goes on only at the address of the case it saved, not at the start screen', async () => {
    // After a demo scenario began from the choice screen, the start address
    // showed that case again instead of the start screen.
    Object.assign(state, { casus, datum: '2026-10-20', scenario: 'A' });
    await verversen();
    Object.assign(state, { casus: null, scenario: null, datum: '' });
    expect(await hervatten(null)).toBe('none');
    expect(state.casus).toBeNull();
    expect(await hervatten('gehandicaptenparkeerkaart')).toBe('none');
    expect(state.casus).toBeNull();
    expect(await hervatten('vakantieverlof')).toBe('resumed');
    expect(state).toMatchObject({ casus, scenario: 'A' });
  });

  it('says whether a game came back, stays saved for the next reload, or is gone', async () => {
    expect(await hervatten()).toBe('none');
    storage.set('poc-spel', '{not json');
    expect(await hervatten('vakantieverlof')).toBe('none');
    expect(storage.has('poc-spel')).toBe(false);
  });
});

describe('stap', () => {
  it('keeps a refusal of the cell, not a cell that did not load or stopped', async () => {
    Object.assign(state, { casus, scenario: 'A', refusals: [] });
    for (const message of ['the cell did not load: 404', 'the cell stopped: unreachable', 'already replaces']) {
      await stap(async () => {
        throw new Error(message);
      });
    }
    expect(state.refusals).toEqual(['already replaces']);
  });

  it('keeps no refusal in free play', async () => {
    Object.assign(state, { casus, scenario: null, refusals: [] });
    await stap(async () => {
      throw new Error('already replaces');
    });
    expect(state.refusals).toEqual([]);
  });
});

describe('hoofdvelden', () => {
  const met = (eigen_velden) => {
    state.casus = { uitkomst: 'verstrekt' };
    state.vormen = { eigen_velden };
  };
  afterEach(() => {
    state.casus = null;
    state.vormen = null;
  });

  it('gives only the outcome of the case for a decision', () => {
    met({ b: ['verstrekt', 'vermeldt', 'nog_een'] });
    const besluit = { id: 'b', fields: { bsn: '1' }, outputs: { verstrekt: true, vermeldt: true, nog_een: 1 } };
    expect(hoofdvelden(besluit)).toEqual(['verstrekt: true']);
  });

  it('gives at most two own fields, and none when there are more', () => {
    met({ twee: ['kenteken', 'kvk'], drie: ['a', 'b', 'c'] });
    expect(hoofdvelden({ id: 'twee', fields: { kenteken: 'AB', kvk: '1', hook: 'x' } })).toEqual(['kenteken: AB', 'kvk: 1']);
    expect(hoofdvelden({ id: 'drie', fields: { a: 1, b: 2, c: 3 } })).toEqual([]);
  });

  it('counts no metadata: empty values, references and copies of a decision', () => {
    const kopie = { basis: 'awb#1', fields: {}, findings: {}, outputs: {} };
    const id = '01a18d9a-0916-72f7-b24a-f062a8205051';
    met({ g: ['leeg', 'ander', 'besluit', 'zaak'] });
    expect(hoofdvelden({ id: 'g', fields: { leeg: null, ander: id, besluit: kopie, zaak: 'x' } })).toEqual(['zaak: x']);
  });

  it('gives nothing for a gram the law gives no fields', () => {
    met({});
    expect(hoofdvelden({ id: 'g', fields: { a: 1 } })).toEqual([]);
  });
});
