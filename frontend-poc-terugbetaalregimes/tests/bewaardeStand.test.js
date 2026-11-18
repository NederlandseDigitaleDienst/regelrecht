import { describe, it, expect, beforeEach, vi } from 'vitest';
import { nextTick } from 'vue';
import { isPersonaWissel, keuzesBijBinnenkomst } from '../src/lib/personaKeuze.js';

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
const PREFIX = 'terugbetaalregimes:v1:';

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

/**
 * De burgerview bewaart de keuzes per persona. De valkuil zat niet in het
 * bewaren maar in het terugzetten: de persona-switcher stuurt ook een
 * select-event als hij zichzelf synchroniseert, en dat werd gelezen als "de
 * gebruiker koos iemand anders", waarna de zojuist teruggezette keuzes weer
 * op de standaard gingen. Deze test legt vast dat alleen een échte wissel
 * telt.
 */
describe('keuzes per persona terugzetten', () => {
  // De stappen van selectPersona in BurgerView, met de echte regels.
  function kies(huidig, nieuw, opslag) {
    if (!isPersonaWissel(huidig, nieuw)) return huidig;
    return { bsn: nieuw.bsn, keuzes: keuzesBijBinnenkomst(nieuw.standaard, opslag[nieuw.bsn]) };
  }

  const a = { bsn: '1', standaard: { overstapAangevraagd: false } };
  const b = { bsn: '2', standaard: { overstapAangevraagd: false } };

  it('houdt de teruggezette keuzes bij een herhaald select-event', () => {
    const opslag = { 1: { overstapAangevraagd: true } };
    let stand = kies(null, a, opslag);
    expect(stand.keuzes.overstapAangevraagd).toBe(true);

    // De switcher synchroniseert en stuurt hetzelfde persona nog eens.
    stand = kies(stand, a, opslag);
    expect(stand.keuzes.overstapAangevraagd).toBe(true);
  });

  it('begint bij een andere persoon met diens eigen uitgangspunt', () => {
    const opslag = { 1: { overstapAangevraagd: true } };
    let stand = kies(null, a, opslag);
    stand = kies(stand, b, opslag);
    expect(stand.bsn).toBe('2');
    expect(stand.keuzes.overstapAangevraagd).toBe(false);
  });
});
