import { beforeEach, describe, expect, it, vi } from 'vitest';
import { hervatten, state, stap, verversen } from '../src/state.js';

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
    expect(await hervatten()).toBe('resumed');
    expect(state).toMatchObject({
      casus,
      datum: '2026-11-16',
      scenario: 'Tweede bezwaar',
      refusals: ['already replaces'],
    });
  });

  it('opens a save from before the guide in free play', async () => {
    storage.set('poc-spel', JSON.stringify({ spel: {}, datum: '2026-10-20' }));
    Object.assign(state, { scenario: 'over', refusals: ['over'] });
    await hervatten();
    expect(state).toMatchObject({ casus, datum: '2026-10-20', scenario: null, refusals: [] });
  });

  it('says whether a game came back, stays saved for the next reload, or is gone', async () => {
    expect(await hervatten()).toBe('none');
    storage.set('poc-spel', '{not json');
    expect(await hervatten()).toBe('none');
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
