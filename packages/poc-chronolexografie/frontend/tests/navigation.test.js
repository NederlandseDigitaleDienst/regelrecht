import { nextTick, reactive } from 'vue';
import { beforeEach, describe, expect, it, vi } from 'vitest';

// The game itself is not played here: choosing a case and beginning a
// scenario only set `state`, as state.js does once the cell agrees.
const casus = {
  id: 'vakantieverlof',
  titel: 'Vakantieverlof',
  scenarios: [{ naam: 'Aanvraag wordt verleend' }, { naam: 'Bezwaar te laat: niet-ontvankelijk' }],
};
const state = reactive({ casus: null, scenario: null, fout: '' });
const calls = [];
vi.mock('../src/state.js', () => ({
  state,
  kiezen: async (id) => {
    calls.push(['kiezen', id]);
    state.casus = { ...casus, id };
    state.scenario = null;
    return true;
  },
  beginScenario: async (naam) => {
    calls.push(['beginScenario', naam]);
    state.scenario = naam;
    return true;
  },
  naarKeuze: () => {
    calls.push(['naarKeuze']);
    state.casus = null;
    state.scenario = null;
  },
}));
vi.mock('../src/api.js', () => ({
  api: { casussen: async () => ({ casussen: [{ id: 'vakantieverlof' }] }) },
}));

// The address bar and the history of one tab.
const listeners = {};
const entries = [];
const location = { pathname: '/', search: '', hash: '' };
const go = (url) => Object.assign(location, { pathname: new URL(url, 'http://x').pathname });
vi.stubGlobal('window', {
  location,
  history: {
    state: null,
    pushState: (_s, _t, url) => {
      entries.push(url);
      go(url);
    },
    replaceState: (_s, _t, url) => go(url),
  },
  addEventListener: (type, fn) => (listeners[type] = fn),
});

const { followAddress, toChoice, unknown } = await import('../src/navigation.js');

const flush = async () => {
  for (let i = 0; i < 5; i++) await nextTick();
};

beforeEach(() => {
  calls.length = 0;
  entries.length = 0;
  Object.assign(state, { casus: null, scenario: null, fout: '' });
});

describe('the address', () => {
  it('opens the scenario a link names, and writes each choice after that', async () => {
    go('/vakantieverlof/bezwaar-te-laat-niet-ontvankelijk');
    const started = vi.fn();
    await followAddress(started);
    expect(calls).toEqual([
      ['kiezen', 'vakantieverlof'],
      ['beginScenario', 'Bezwaar te laat: niet-ontvankelijk'],
    ]);
    expect(started).toHaveBeenCalledOnce();
    expect(entries).toEqual([]);

    state.scenario = 'Aanvraag wordt verleend';
    await flush();
    expect(entries).toEqual(['/vakantieverlof/aanvraag-wordt-verleend']);

    state.casus = null;
    state.scenario = null;
    await flush();
    expect(entries.at(-1)).toBe('/');
  });

  it('follows the browser back to the case, keeping it chosen', async () => {
    Object.assign(state, { casus, scenario: 'Aanvraag wordt verleend' });
    go('/vakantieverlof');
    await listeners.popstate();
    expect(state.casus.id).toBe('vakantieverlof');
    expect(state.scenario).toBeNull();
    expect(calls).toEqual([]);
  });

  it('follows the browser back to the choice screen', async () => {
    Object.assign(state, { casus });
    go('/');
    await listeners.popstate();
    expect(calls).toEqual([['naarKeuze']]);
  });

  it('names an unknown case or scenario, and leaves its address in the bar', async () => {
    go('/bestaat-niet');
    await listeners.popstate();
    expect(unknown.value).toEqual({ casus: 'bestaat-niet' });
    expect(location.pathname).toBe('/bestaat-niet');

    go('/vakantieverlof/bestaat-niet');
    await listeners.popstate();
    expect(unknown.value).toEqual({ casus: 'Vakantieverlof', scenario: 'bestaat-niet' });
    expect(location.pathname).toBe('/vakantieverlof/bestaat-niet');

    await toChoice();
    expect(unknown.value).toBeNull();
    expect(location.pathname).toBe('/');
  });
});
