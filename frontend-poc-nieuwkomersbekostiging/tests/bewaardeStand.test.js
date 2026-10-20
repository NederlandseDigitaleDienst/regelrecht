import { describe, it, expect, beforeEach, vi } from 'vitest';
import { nextTick } from 'vue';

/**
 * Wat de bewaarde stand doet staat getest bij de gedeelde module
 * (packages/frontend-shared/src/usePersistedState.test.ts). Wat hier overblijft
 * is wat deze casus zelf regelt: de sleutelprefix, via de shim in
 * src/composables.
 */
function verseOpslag() {
  const data = new Map();
  return {
    getItem: (k) => (data.has(k) ? data.get(k) : null),
    setItem: (k, v) => data.set(k, String(v)),
    removeItem: (k) => data.delete(k),
    get length() { return data.size; },
    key: (i) => [...data.keys()][i],
    _data: data,
  };
}

/**
 * De sleutelprefix van deze casus. Staat hier letterlijk en niet via een
 * import: als de prefix verandert, verliest iedereen zijn bewaarde keuzes, en
 * dat hoort een test te merken in plaats van mee te bewegen.
 */
const PREFIX = 'nieuwkomersbekostiging:v1:';

async function laad() {
  vi.resetModules();
  return import('../src/composables/useBewaardeStand.js');
}

describe('bewaarde stand', () => {
  beforeEach(() => {
    const opslag = verseOpslag();
    // Object.keys() over localStorage werkt in de browser via de indexer;
    // in de test bootsen we dat na met echte eigenschappen.
    vi.stubGlobal('localStorage', new Proxy(opslag, {
      ownKeys: () => [...opslag._data.keys()],
      getOwnPropertyDescriptor: () => ({ enumerable: true, configurable: true }),
    }));
    vi.stubGlobal('window', { location: { reload: vi.fn() } });
  });

  // De twee OCW-pocs draaien hosted op dezelfde origin achter het portaal, dus
  // een gedeelde prefix zou betekenen dat ze elkaars keuzes lezen.
  it('schrijft onder de prefix van deze casus', async () => {
    const { persistedRef } = await laad();
    persistedRef('n', 500).value = 900;
    await nextTick();

    expect(localStorage.getItem(`${PREFIX}n`)).toBe('900');
    expect(Object.keys(localStorage).every((k) => k.startsWith(PREFIX))).toBe(true);
  });
});
