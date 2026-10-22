import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { Mock } from 'vitest';
import { mount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import SearchPopover from './SearchPopover.vue';
import { MIN_QUERY_LENGTH } from '../composables/useBwbSearch';
import type { CorpusLaw } from '../composables/useCorpusLaws';
import { enrichedOnly } from '../composables/useLawOverview';

/** The part of a Response apiFetch reads; the stubs return only this. */
interface StubResponse {
  ok: boolean;
  status?: number;
  json?: () => Promise<unknown>;
}
type FetchStub = (url: string | URL | Request, init?: RequestInit) => Promise<StubResponse>;

/** The stubbed global fetch, typed as the stub it is rather than as `fetch`. */
const fetchMock = () => vi.mocked(fetch) as unknown as Mock<FetchStub>;

/**
 * Setup state the tests reach through `wrapper.vm`. Only `show` is exposed;
 * test-utils proxies the rest of the setup state onto `vm`.
 */
interface SearchPopoverInternals {
  search: string;
  sortedLaws: CorpusLaw[];
  searchFailed: boolean;
  useCenteredPosition: boolean;
  isAnchored: boolean;
}
const internals = (wrapper: VueWrapper) => wrapper.vm as unknown as SearchPopoverInternals;

/** The (not upgraded, under happy-dom) popover element, with the members the tests replace. */
interface PopoverStub {
  show: () => void;
  hide: () => void;
  matches: (selector: string) => boolean;
  reposition: () => unknown;
  anchorElement: Element | null;
}
const popoverEl = (wrapper: VueWrapper) => wrapper.find('nldd-popover').element as PopoverStub;

// SearchPopover queries the corpus server-side (`/corpus/laws?q=`) and orders
// the response into a flat option list (private repo first). We drive it via
// `wrapper.vm` and assert on the `sortedLaws` computed.
//
// useTrajects() calls vue-router's useRoute(); mock it so the component mounts
// without a router. Default: no active traject → global /api/corpus URL. The
// traject-scoped tests swap in a trajectRef per test (vi.hoisted so the
// hoisted mock factory can reference it).
const routeMock = vi.hoisted((): { params: Record<string, string> } => ({ params: {} }));
vi.mock('vue-router', () => ({
  useRoute: () => routeMock,
}));

// Central corpus (priority 2) listed before the private traject repo
// (priority 0) to prove the sort order comes from source_priority, not
// response order. Source name/law id are anonymized fixtures, not real repos.
const LAWS: CorpusLaw[] = [
  {
    law_id: 'besluit_zorgverzekering',
    source_id: 'central',
    source_name: 'Centrale Regelrecht Corpus',
    source_priority: 2,
  },
  {
    law_id: 'besluit_zorgverzekering_example',
    source_id: 'traject-own',
    source_name: 'example-org/regelrecht-corpus-example',
    source_priority: 0,
  },
];

beforeEach(() => {
  routeMock.params = {};
  enrichedOnly.value = false;
  vi.stubGlobal(
    'fetch',
    vi.fn<FetchStub>(async (url) => {
      const u = String(url);
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      if (u.includes('/corpus/laws') && u.includes('q=')) {
        return { ok: true, json: async () => LAWS };
      }
      return { ok: true, json: async () => [] };
    }),
  );
});

// Wait past the 200ms input debounce plus the awaited fetch.
const settle = () => new Promise((r) => setTimeout(r, 300));

describe('SearchPopover server-side search', () => {
  it('orders corpus matches private repo (priority 0) first, source per row', async () => {
    const wrapper = mount(SearchPopover);
    internals(wrapper).search = 'zorgverzekering';
    await nextTick();
    await settle();
    await nextTick();

    // Flat option list sorted by source_priority: the private traject repo
    // (priority 0) sorts above the central corpus (priority 2). Each row keeps
    // its own source_name (shown as supporting-text in the listbox).
    const laws = internals(wrapper).sortedLaws;
    expect(laws.map((l) => l.law_id)).toEqual([
      'besluit_zorgverzekering_example',
      'besluit_zorgverzekering',
    ]);
    expect(laws.map((l) => l.source_name)).toEqual([
      'example-org/regelrecht-corpus-example',
      'Centrale Regelrecht Corpus',
    ]);
  });

  it('queries the backend with the q parameter (not a client-side filter)', async () => {
    const wrapper = mount(SearchPopover);
    internals(wrapper).search = 'kieswet';
    await nextTick();
    await settle();

    const urls = fetchMock().mock.calls.map((c) => String(c[0]));
    expect(urls.some((u) => u.includes('/corpus/laws') && u.includes('q=kieswet'))).toBe(true);
  });

  it('debounces: a single query fires one corpus request', async () => {
    const wrapper = mount(SearchPopover);
    internals(wrapper).search = 'z';
    await nextTick();
    internals(wrapper).search = 'zo';
    await nextTick();
    internals(wrapper).search = 'zorg';
    await nextTick();
    await settle();

    const corpusCalls = fetchMock().mock.calls
      .map((c) => String(c[0]))
      .filter((u) => u.includes('/corpus/laws') && u.includes('q='));
    expect(corpusCalls).toHaveLength(1);
    expect(corpusCalls[0]).toContain('q=zorg');
  });

  it('clearing the query discards an in-flight corpus fetch (no stale results)', async () => {
    // Make the corpus fetch hang so we can clear the input mid-flight.
    let resolveCorpus: (() => void) | undefined;
    fetchMock().mockImplementation((url) => {
      const u = String(url);
      if (u.includes('/auth/status')) return Promise.resolve({ ok: false, json: async () => ({}) });
      if (u.includes('/corpus/laws') && u.includes('q=')) {
        return new Promise<StubResponse>((r) => {
          resolveCorpus = () => r({ ok: true, json: async () => LAWS });
        });
      }
      return Promise.resolve({ ok: true, json: async () => [] });
    });

    const wrapper = mount(SearchPopover);
    internals(wrapper).search = 'zorg';
    await nextTick();
    await settle(); // debounce fires; the corpus fetch is now pending

    // Clear before the fetch resolves, then let the stale fetch settle.
    internals(wrapper).search = '';
    await nextTick();
    resolveCorpus!(); // set when the pending corpus fetch started
    await settle();
    await nextTick();

    // The cleared term's response must not repopulate the list, and no
    // wetten.overheid.nl search should have been fired for it.
    expect(internals(wrapper).sortedLaws).toEqual([]);
    const bwbCalls = fetchMock().mock.calls
      .map((c) => String(c[0]))
      .filter((u) => u.includes('/harvest/search'));
    expect(bwbCalls).toHaveLength(0);
  });

  it('a backend error surfaces as failed, not a cascade to the external fallback', async () => {
    fetchMock().mockImplementation((url) => {
      const u = String(url);
      if (u.includes('/auth/status')) return Promise.resolve({ ok: false, json: async () => ({}) });
      if (u.includes('/corpus/laws') && u.includes('q=')) {
        return Promise.resolve({ ok: false, status: 500, json: async () => [] });
      }
      return Promise.resolve({ ok: true, json: async () => [] });
    });

    const wrapper = mount(SearchPopover);
    internals(wrapper).search = 'zorg';
    await nextTick();
    await settle();
    await nextTick();

    expect(internals(wrapper).searchFailed).toBe(true);
    expect(internals(wrapper).sortedLaws).toEqual([]);
    const bwbCalls = fetchMock().mock.calls
      .map((c) => String(c[0]))
      .filter((u) => u.includes('/harvest/search'));
    expect(bwbCalls).toHaveLength(0);
  });

  it('renders no promote buttons outside a traject (global corpus scope)', async () => {
    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, 'zorgverzekering');

    expect(wrapper.findAll('nldd-list-item[data-law-id]')).toHaveLength(2);
    expect(promoteButtons(wrapper)).toHaveLength(0);
  });
});

// Fictieve traject-ref (publiek testbestand, geen echte repo/traject-namen).
const TRAJECT_REF = 'voorbeeld-abcd1234';

async function searchFor(wrapper: VueWrapper, term: string) {
  internals(wrapper).search = term;
  await nextTick();
  await settle();
  await nextTick();
}

function promoteButtons(wrapper: VueWrapper) {
  return wrapper
    .findAll('nldd-button')
    .filter((b) => b.attributes('text') === 'Toevoegen aan traject');
}

// De "Toevoegen aan traject"-knop is bewust uit de zoekresultaten verwijderd:
// promoten van een wet uit het centrale corpus kan uitsluitend via de
// expliciete "Wet toevoegen"-flow (AddLawSheet). Deze tests pinnen dat de
// knop óók binnen een traject niet (terug)verschijnt.
describe('SearchPopover zonder "Toevoegen aan traject" in zoekresultaten', () => {
  beforeEach(() => {
    routeMock.params = { trajectRef: TRAJECT_REF };
  });

  it('toont binnen een traject geen promote-knop; de ondertitel is de bron', async () => {
    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, 'zorgverzekering');

    const rows = wrapper.findAll('nldd-list-item[data-law-id]');
    expect(rows).toHaveLength(2);
    // Ook de centrale-seed-treffer (priority 2, niet in de eigen repo) krijgt
    // geen knop meer — promoten kan alleen nog via de AddLawSheet.
    expect(promoteButtons(wrapper)).toHaveLength(0);

    const central = rows.find((r) => r.attributes('data-law-id') === 'besluit_zorgverzekering');
    expect(central).toBeDefined();
    expect(central!.get('nldd-text-cell').attributes('supporting-text')).toBe(
      'Centrale Regelrecht Corpus',
    );
  });

  it('een rij-klik navigeert alleen (select-law), er wordt niets gepromoot', async () => {
    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, 'zorgverzekering');

    await wrapper
      .get('nldd-list-item[data-law-id="besluit_zorgverzekering"]')
      .trigger('click');
    // select-law wordt uitgesteld tot de popover dicht is (deferral).
    await wrapper.get('nldd-popover').trigger('close');

    expect(wrapper.emitted('select-law')).toEqual([['besluit_zorgverzekering']]);
    expect(wrapper.emitted('promoted')).toBeUndefined();
    const promoteCalls = fetchMock().mock.calls.filter(
      (c) => String(c[0]).includes('/promote') && c[1]?.method === 'POST',
    );
    expect(promoteCalls).toHaveLength(0);
  });
});

describe('SearchPopover: tweede klik op de trigger sluit', () => {
  // De triggers staan in de app-shell en deze popover in de routed view, dus
  // de knop is niet de invoker van de popover en de browser levert geen toggle.
  // show() doet dat zelf; zonder die stap opende een tweede klik alleen opnieuw
  // en leek er niets te gebeuren.
  function mountMetPopoverStub() {
    const wrapper = mount(SearchPopover);
    const el = popoverEl(wrapper);
    const state = { open: false };
    el.show = () => {
      state.open = true;
    };
    el.hide = () => {
      state.open = false;
    };
    // happy-dom kent :popover-open niet; stub 'm zoals het echte component 'm zou melden.
    el.matches = (selector) => selector === ':popover-open' && state.open;
    return { wrapper, state };
  }

  it('sluit bij een tweede show() zonder zoektekst', async () => {
    const { wrapper, state } = mountMetPopoverStub();
    await wrapper.vm.show(document.createElement('button'));
    expect(state.open).toBe(true);
    await wrapper.vm.show(document.createElement('button'));
    expect(state.open).toBe(false);
  });

  it('houdt hem open bij type-to-open, want dat is geen her-trigger', async () => {
    const { wrapper, state } = mountMetPopoverStub();
    await wrapper.vm.show(document.createElement('button'));
    await wrapper.vm.show(document.createElement('button'), 'a');
    expect(state.open).toBe(true);
  });
});

describe('SearchPopover volgt het breakpoint terwijl hij openstaat', () => {
  // Zonder dit blijft een open popover in de layout van het vorige breakpoint
  // hangen: een lg-gecentreerd paneel dat op md los in beeld zweeft, of een
  // md-geankerd paneel dat nog naar een knop wijst die inmiddels het brede
  // zoekveld is. Sluiten bij resize zou simpeler zijn maar gooit weg wat de
  // gebruiker aan het typen was.
  function stubMatchMedia(breedte: { waarde: number }) {
    const luisteraars = new Set<() => void>();
    vi.stubGlobal('matchMedia', (query: string) => {
      const min = Number(/min-width:\s*(\d+)/.exec(query)?.[1] ?? 0);
      const max = Number(/max-width:\s*(\d+)/.exec(query)?.[1] ?? Infinity);
      const mql = {
        get matches() {
          return breedte.waarde >= min && breedte.waarde <= max;
        },
        addEventListener: (_: string, fn: () => void) => luisteraars.add(fn),
        removeEventListener: (_: string, fn: () => void) => luisteraars.delete(fn),
      };
      return mql;
    });
    return { verander: (nieuw: number) => {
      breedte.waarde = nieuw;
      for (const fn of luisteraars) fn();
    } };
  }

  it('wisselt van gecentreerd naar geankerd als lg naar md gaat', async () => {
    const breedte = { waarde: 1280 };
    const { verander } = stubMatchMedia(breedte);
    const wrapper = mount(SearchPopover);
    const el = popoverEl(wrapper);
    el.show = () => {};
    el.matches = () => true;
    el.reposition = vi.fn();

    await wrapper.vm.show(document.createElement('button'));
    expect(internals(wrapper).useCenteredPosition).toBe(true);
    expect(internals(wrapper).isAnchored).toBe(false);

    verander(768);
    await nextTick();
    expect(internals(wrapper).useCenteredPosition).toBe(false);
    expect(internals(wrapper).isAnchored).toBe(true);
    expect(el.reposition).toHaveBeenCalled();
  });

  it('herankert naar de trigger die in het nieuwe breakpoint zichtbaar is', async () => {
    // Elk viewport toont zijn eigen trigger en verbergt de andere. Zonder
    // herankeren wijst de popover na een resize naar een element van nul bij
    // nul en parkeert Floating UI 'm linksboven.
    const verborgen = document.createElement('div');
    verborgen.setAttribute('data-search-trigger', '');
    // Only width and height are read (visibleTrigger, syncToViewport).
    verborgen.getBoundingClientRect = () => ({ width: 0, height: 0 }) as DOMRect;
    const zichtbaar = document.createElement('div');
    zichtbaar.setAttribute('data-search-trigger', '');
    zichtbaar.getBoundingClientRect = () => ({ width: 120, height: 40 }) as DOMRect;
    document.body.append(verborgen, zichtbaar);

    const breedte = { waarde: 1280 };
    const { verander } = stubMatchMedia(breedte);
    const wrapper = mount(SearchPopover);
    const el = popoverEl(wrapper);
    const staat = { open: false };
    el.show = () => {
      staat.open = true;
    };
    el.matches = (selector) => selector === ':popover-open' && staat.open;
    el.reposition = vi.fn();

    await wrapper.vm.show(verborgen);
    expect(el.anchorElement).toBe(verborgen);

    verander(768);
    await nextTick();
    expect(el.anchorElement).toBe(zichtbaar);

    verborgen.remove();
    zichtbaar.remove();
  });

  it('laat een gesloten popover met rust', async () => {
    const breedte = { waarde: 1280 };
    const { verander } = stubMatchMedia(breedte);
    const wrapper = mount(SearchPopover);
    const el = popoverEl(wrapper);
    el.matches = () => false;
    el.reposition = vi.fn();

    verander(768);
    await nextTick();
    expect(el.reposition).not.toHaveBeenCalled();
  });
});

// De hint zei "minimaal twee letters" terwijl er pas vanaf drie tekens gezocht
// wordt (MIN_QUERY_LENGTH, en de pipeline-api weigert korter ook). Drie is de
// bedoeling: de constante, de AddLawSheet-hint en de backend zeggen het alle
// drie. Deze tests binden de tekst aan het gedrag.
describe('SearchPopover: minimale zoeklengte', () => {
  const WORDS: Record<number, string> = { 3: 'drie' };

  function emptyTexts(wrapper: VueWrapper) {
    return wrapper.findAll('nldd-inline-dialog').map((d) => d.attributes('text'));
  }

  function corpusCalls() {
    return fetchMock().mock.calls
      .map(([u]) => String(u))
      .filter((u) => u.includes('/corpus/laws') && u.includes('q='));
  }

  it('noemt het aantal tekens waarbij het zoeken echt begint', async () => {
    expect(WORDS[MIN_QUERY_LENGTH], `voeg ${MIN_QUERY_LENGTH} toe aan WORDS`).toBeDefined();
    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, 'a'.repeat(MIN_QUERY_LENGTH - 1));
    expect(corpusCalls()).toEqual([]);
    expect(emptyTexts(wrapper)).toContain(`Typ minimaal ${WORDS[MIN_QUERY_LENGTH]} tekens om te zoeken`);
  });

  it('zoekt vanaf dat aantal, en toont de hint dan niet meer', async () => {
    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, 'a'.repeat(MIN_QUERY_LENGTH));
    expect(corpusCalls()).toHaveLength(1);
    expect(emptyTexts(wrapper).some((t) => t?.startsWith('Typ minimaal'))).toBe(false);
  });

  it('telt spaties aan het eind niet mee, net als het zoeken zelf', async () => {
    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, `${'a'.repeat(MIN_QUERY_LENGTH - 1)}   `);
    expect(corpusCalls()).toEqual([]);
    expect(emptyTexts(wrapper)).toContain(`Typ minimaal ${WORDS[MIN_QUERY_LENGTH]} tekens om te zoeken`);
  });
});

describe('SearchPopover: only enriched laws', () => {
  const corpusUrls = () =>
    fetchMock().mock.calls.map((c) => String(c[0])).filter((u) => u.includes('/corpus/laws'));

  it('sends the filter along with the search term', async () => {
    const wrapper = mount(SearchPopover);
    enrichedOnly.value = true;
    internals(wrapper).search = 'zorgtoeslag';
    await nextTick();
    await settle();
    expect(corpusUrls().some((u) => u.includes('q=zorgtoeslag') && u.includes('enriched=true'))).toBe(true);
  });

  it('lists the enriched laws when there is no search term', async () => {
    mount(SearchPopover);
    enrichedOnly.value = true;
    await nextTick();
    await settle();
    expect(corpusUrls().some((u) => !u.includes('q=') && u.includes('enriched=true'))).toBe(true);
  });

  it('does not fall back to wetten.overheid.nl when no enriched law matches', async () => {
    fetchMock().mockImplementation(async (url) => {
      const u = String(url);
      if (u.includes('/auth/status')) return { ok: true, json: async () => ({ authenticated: true }) };
      return { ok: true, json: async () => [] };
    });
    const wrapper = mount(SearchPopover);
    enrichedOnly.value = true;
    internals(wrapper).search = 'kieswet';
    await nextTick();
    await settle();
    await nextTick();
    const bwbCalls = fetchMock().mock.calls.map((c) => String(c[0])).filter((u) => u.includes('/harvest/search'));
    expect(bwbCalls).toHaveLength(0);
  });
});
