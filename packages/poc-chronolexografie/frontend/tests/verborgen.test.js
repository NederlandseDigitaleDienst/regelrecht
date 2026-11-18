import { describe, expect, it } from 'vitest';
import {
  PARAM,
  SLEUTEL,
  createAlleCasussen,
  gevraagd,
  verborgenNamen,
  zichtbareBronnen,
  zichtbareCasussen,
} from '../src/verborgen.js';

const casussen = [{ id: 'eigen' }, { id: 'open', bron: 'open' }, { id: 'dicht', bron: 'dicht' }];
const lijst = [
  { naam: 'open', verborgen: false },
  { naam: 'dicht', verborgen: true },
];

/** A window with an address bar and a `localStorage`, enough for `follow`. */
function venster(href, opgeslagen = {}) {
  const store = { ...opgeslagen };
  const win = {
    location: new URL(href),
    history: {
      state: null,
      replaceState: (_s, _t, url) => {
        win.location = new URL(url);
      },
    },
    localStorage: {
      getItem: (k) => store[k] ?? null,
      setItem: (k, v) => (store[k] = String(v)),
      removeItem: (k) => delete store[k],
    },
  };
  return { win, store };
}

describe('hidden sources', () => {
  it('leave their cases and their notices off the choice screen', () => {
    const verborgen = verborgenNamen(lijst);
    expect(verborgen).toEqual(['dicht']);
    expect(zichtbareCasussen(casussen, verborgen).map((c) => c.id)).toEqual(['eigen', 'open']);
    const bronnen = [{ naam: 'open' }, { naam: 'dicht', fout: 'bron niet beschikbaar' }];
    expect(zichtbareBronnen(bronnen, verborgen).map((b) => b.naam)).toEqual(['open']);
  });

  it('show everything once this browser asks for all cases', () => {
    const verborgen = verborgenNamen(lijst);
    expect(zichtbareCasussen(casussen, verborgen, true)).toEqual(casussen);
    expect(zichtbareBronnen([{ naam: 'dicht' }], verborgen, true)).toEqual([{ naam: 'dicht' }]);
  });
});

describe(`?${PARAM}`, () => {
  it('reads on, off, or nothing', () => {
    expect(gevraagd(`?${PARAM}`)).toBe(true);
    expect(gevraagd(`?${PARAM}=ja`)).toBe(true);
    expect(gevraagd(`?${PARAM}=uit`)).toBe(false);
    expect(gevraagd('?presentatie')).toBeNull();
  });

  it('switches the hidden cases on, remembers it, and leaves the address', () => {
    const { win, store } = venster(`https://x/chronolexografie/?${PARAM}&presentatie=2`);
    const a = createAlleCasussen(win);
    expect(a.alles.value).toBe(false);
    a.follow();
    expect(a.alles.value).toBe(true);
    expect(store[SLEUTEL]).toBe('ja');
    expect(win.location.href).toBe('https://x/chronolexografie/?presentatie=2');

    // A next visit, without the parameter, still shows them.
    expect(createAlleCasussen(venster('https://x/chronolexografie/', store).win).alles.value).toBe(true);
  });

  it('switches them off again with =uit', () => {
    const { win, store } = venster(`https://x/chronolexografie/dicht?${PARAM}=uit`, { [SLEUTEL]: 'ja' });
    const a = createAlleCasussen(win);
    expect(a.alles.value).toBe(true);
    a.follow();
    expect(a.alles.value).toBe(false);
    expect(store[SLEUTEL]).toBeUndefined();
    expect(win.location.href).toBe('https://x/chronolexografie/dicht');
  });

  it('leaves an address without the parameter alone', () => {
    const { win } = venster('https://x/chronolexografie/?uitleg');
    createAlleCasussen(win).follow();
    expect(win.location.href).toBe('https://x/chronolexografie/?uitleg');
  });

  it('still works for the page when storage throws', () => {
    const { win } = venster(`https://x/?${PARAM}`);
    const kapot = () => {
      throw new Error('blocked');
    };
    win.localStorage = { getItem: kapot, setItem: kapot, removeItem: kapot };
    const a = createAlleCasussen(win);
    expect(a.alles.value).toBe(false);
    a.follow();
    expect(a.alles.value).toBe(true);

    const zonder = venster('https://x/').win;
    Object.defineProperty(zonder, 'localStorage', { get: kapot });
    expect(createAlleCasussen(zonder).alles.value).toBe(false);
  });
});
