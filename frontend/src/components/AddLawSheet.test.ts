import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { Mock } from 'vitest';
import { mount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import AddLawSheet from './AddLawSheet.vue';
import type { CorpusLaw } from '../corpus';
import type { MemoryTransport } from '../corpus/memoryTransport';
import { installTestCorpus } from '../test-utils/corpusSetup';
import type { BwbSearchResult } from '../composables/useBwbSearch';

// De sheet is traject-gebonden: useTrajects() leest de trajectRef uit de
// route. Fictieve ref, geen echte repo/traject-namen (publiek testbestand).
const TRAJECT_REF = 'voorbeeld-abcd1234';

vi.mock('vue-router', () => ({
  useRoute: () => ({ params: { trajectRef: TRAJECT_REF } }),
}));

// Traject-scoped zoekrespons: het traject federeert de centrale seed, dus de
// lijst mengt eigen-repo-treffers (source_priority 0, al in het traject) met
// centrale-seed-treffers (hogere priority, promoteerbaar). Fictieve namen.
const SEARCH_LAWS: CorpusLaw[] = [
  {
    law_id: 'wet_voorbeeld',
    name: 'Wet voorbeeld',
    source_id: 'central',
    source_name: 'Centrale Regelrecht Corpus',
    source_priority: 2,
  },
  {
    law_id: 'wet_al_binnen',
    name: 'Wet al binnen',
    source_id: 'traject-own',
    source_name: 'example-org/traject-corpus-example',
    source_priority: 0,
  },
];

const PROMOTABLE = SEARCH_LAWS[0]!; // a literal two-element array

/** The part of a Response apiFetch reads; the stubs return only this. */
interface StubResponse {
  ok: boolean;
  status?: number;
  json?: () => Promise<unknown>;
  text?: () => Promise<string>;
  headers?: { get: () => string };
}
type FetchStub = (url: string | URL | Request, init?: RequestInit) => Promise<StubResponse>;

/** The stubbed global fetch, typed as the stub it is rather than as `fetch`. */
const fetchMock = () => vi.mocked(fetch) as unknown as Mock<FetchStub>;

/**
 * Setup state the tests reach through `wrapper.vm`. It is not exposed (only
 * `show` is), but test-utils proxies the setup state onto `vm`.
 */
interface AddLawSheetInternals {
  search: string;
  mode: 'search' | 'upload';
  sortedLaws: CorpusLaw[];
  bwbIdQuery: string | null;
  harvestState: Record<string, string>;
  promote(law: CorpusLaw): Promise<void>;
  isInTraject(law: CorpusLaw): boolean;
  requestTrajectHarvest(bwbId: string, lawName?: string): Promise<void>;
  $refs: { sheetRef: { hide: () => void } };
}
const internals = (wrapper: VueWrapper) => wrapper.vm as unknown as AddLawSheetInternals;

/** The in-memory corpus the sheet searches; its `calls` show which corpus was read. */
let corpusTransport: MemoryTransport;

// The search reads the traject's corpus (in memory); the global fetch stub
// answers auth, the external wetten.overheid.nl search, promote and harvest.
function stubFetch(
  { searchLaws = SEARCH_LAWS, bwbResults = [] }: { searchLaws?: CorpusLaw[]; bwbResults?: BwbSearchResult[] } = {},
) {
  corpusTransport = installTestCorpus({ '': {}, [TRAJECT_REF]: { list: searchLaws } }, TRAJECT_REF).transport;
  vi.stubGlobal(
    'fetch',
    vi.fn<FetchStub>(async (url, init = {}) => {
      const u = String(url);
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      if (u.includes('/harvest/search')) return { ok: true, json: async () => bwbResults };
      if (u.includes('/promote') && init.method === 'POST') {
        return { ok: true, json: async () => ({ law_id: 'wet_voorbeeld', copied_files: 3 }) };
      }
      if (u.includes('/corpus/harvest') && init.method === 'POST') {
        return { ok: true, status: 202, json: async () => ({ job_id: 'j1' }) };
      }
      return { ok: true, json: async () => [] };
    }),
  );
}

beforeEach(() => {
  stubFetch();
});

// Voorbij de 200ms-debounce plus de fetches.
const settle = () => new Promise((r) => setTimeout(r, 300));

async function searchFor(wrapper: VueWrapper, term: string) {
  internals(wrapper).search = term;
  await nextTick();
  await settle();
  await nextTick();
}

describe('AddLawSheet', () => {
  it('zoekt via de traject-scoped corpus-API en sorteert eigen repo eerst', async () => {
    const wrapper = mount(AddLawSheet);
    await searchFor(wrapper, 'wet');

    // Only the traject's corpus was read, not the global one.
    expect(corpusTransport.calls).toEqual([`${TRAJECT_REF} list`]);
    expect(internals(wrapper).sortedLaws.map((l) => l.law_id)).toEqual([
      'wet_al_binnen',
      'wet_voorbeeld',
    ]);
  });

  it('markeert een wet die al in de traject-repo staat (priority 0) als niet-promoteerbaar', async () => {
    const wrapper = mount(AddLawSheet);
    await searchFor(wrapper, 'wet');

    const rows = wrapper.findAll('nldd-list-item[data-law-id]');
    const binnen = rows.find((r) => r.attributes('data-law-id') === 'wet_al_binnen');
    expect(binnen).toBeDefined();
    // Defined: asserted on the line above (and below for buiten).
    expect(binnen!.get('nldd-text-cell').attributes('supporting-text')).toBe('Al in dit traject');
    expect(binnen!.get('nldd-button').attributes('disabled')).toBeDefined();

    const buiten = rows.find((r) => r.attributes('data-law-id') === 'wet_voorbeeld');
    expect(buiten).toBeDefined();
    expect(buiten!.get('nldd-button').attributes('disabled')).toBeUndefined();
  });

  it('promoot via POST /promote en emit "promoted"', async () => {
    const wrapper = mount(AddLawSheet);
    await searchFor(wrapper, 'wet');

    await internals(wrapper).promote(PROMOTABLE);

    const promoteCall = fetchMock().mock.calls.find(
      (c) => String(c[0]).includes('/promote') && c[1]?.method === 'POST',
    );
    expect(promoteCall).toBeDefined();
    expect(String(promoteCall![0])).toBe(
      `/api/trajects/${TRAJECT_REF}/corpus/laws/wet_voorbeeld/promote`,
    );
    expect(wrapper.emitted('promoted')).toEqual([['wet_voorbeeld']]);
  });

  it('een 409 op promote markeert de wet alsnog als al-in-traject', async () => {
    stubFetch({ searchLaws: [] });
    fetchMock().mockImplementation(async (url, init = {}) => {
      const u = String(url);
      if (u.includes('/promote') && init.method === 'POST') {
        return { ok: false, status: 409, text: async () => 'al in traject', headers: { get: () => '' } };
      }
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      return { ok: true, json: async () => [] };
    });

    const wrapper = mount(AddLawSheet);
    await searchFor(wrapper, 'wet');
    await internals(wrapper).promote(PROMOTABLE);
    await nextTick();

    expect(wrapper.emitted('promoted')).toBeUndefined();
    expect(internals(wrapper).isInTraject(PROMOTABLE)).toBe(true);
  });

  it('valt bij nul corpus-treffers terug op de wetten.overheid.nl-zoeker', async () => {
    stubFetch({
      searchLaws: [],
      bwbResults: [{ bwb_id: 'BWBR0002399', title: 'Voorbeeldwet extern', type: 'wet' }],
    });
    const wrapper = mount(AddLawSheet);
    await searchFor(wrapper, 'voorbeeldwet');
    // Debounce van useBwbSearch (400ms) na de corpus-respons.
    await new Promise((r) => setTimeout(r, 450));
    await nextTick();

    const urls = fetchMock().mock.calls.map((c) => String(c[0]));
    expect(urls.some((u) => u.includes('/harvest/search'))).toBe(true);
    const row = wrapper.get('nldd-list-item[data-bwb-id="BWBR0002399"]');
    expect(row.get('nldd-text-cell').attributes('text')).toBe('Voorbeeldwet extern');
  });

  // The search reads the traject's corpus, so a failed search is about this
  // traject's library, not the central corpus. The message must say so too,
  // even before the traject list (and so its name) has arrived: the scope
  // follows the ref in the route, not the fetched traject object.
  it('wijst bij een mislukte zoekopdracht naar het traject, niet naar het centrale corpus', async () => {
    installTestCorpus({ '': {}, [TRAJECT_REF]: { failures: { list: 500 } } }, TRAJECT_REF);
    const wrapper = mount(AddLawSheet);
    await searchFor(wrapper, 'wet');

    const dialog = wrapper.get('nldd-inline-dialog[text="Zoeken is mislukt"]');
    const supporting = dialog.attributes('supporting-text');
    expect(supporting).toContain('bibliotheek van dit traject');
    expect(supporting).not.toContain('centrale corpus');
  });

  it('start een traject-scoped harvest voor een BWB-resultaat en emit "harvest-requested"', async () => {
    const wrapper = mount(AddLawSheet);
    await internals(wrapper).requestTrajectHarvest('BWBR0002399', 'Voorbeeldwet extern');

    const call = fetchMock().mock.calls.find(
      (c) => String(c[0]).includes('/corpus/harvest') && c[1]?.method === 'POST',
    );
    expect(call).toBeDefined();
    expect(String(call![0])).toBe(`/api/trajects/${TRAJECT_REF}/corpus/harvest`);
    // The component sends a JSON string body.
    expect(JSON.parse(call![1]!.body as string)).toEqual({
      bwb_id: 'BWBR0002399',
      law_name: 'Voorbeeldwet extern',
    });
    expect(wrapper.emitted('harvest-requested')).toEqual([['BWBR0002399']]);
    expect(internals(wrapper).harvestState.BWBR0002399).toBe('requested');
  });

  it('een 409 op de harvest-aanvraag toont "er loopt al een aanvraag"', async () => {
    fetchMock().mockImplementation(async (url, init = {}) => {
      const u = String(url);
      if (u.includes('/corpus/harvest') && init.method === 'POST') {
        return { ok: false, status: 409, text: async () => 'loopt al', headers: { get: () => '' } };
      }
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      return { ok: true, json: async () => [] };
    });
    const wrapper = mount(AddLawSheet);
    await internals(wrapper).requestTrajectHarvest('BWBR0002399');
    expect(internals(wrapper).harvestState.BWBR0002399).toBe('conflict');
    expect(wrapper.emitted('harvest-requested')).toBeUndefined();
  });

  it('herkent een direct getypt BWB-id (ook lowercase) als harvest-kandidaat', async () => {
    stubFetch({ searchLaws: [] });
    const wrapper = mount(AddLawSheet);
    await searchFor(wrapper, 'bwbr0002399');
    expect(internals(wrapper).bwbIdQuery).toBe('BWBR0002399');

    await searchFor(wrapper, 'wet op het voortgezet onderwijs');
    expect(internals(wrapper).bwbIdQuery).toBe(null);
  });

  it('toont één rij wanneer de externe zoeker het getypte BWB-id ook vindt', async () => {
    stubFetch({
      searchLaws: [],
      bwbResults: [{ bwb_id: 'BWBR0002399', title: 'Voorbeeldwet extern', type: 'wet' }],
    });
    const wrapper = mount(AddLawSheet);
    await searchFor(wrapper, 'BWBR0002399');
    // Debounce van useBwbSearch (400ms) na de corpus-respons.
    await new Promise((r) => setTimeout(r, 450));
    await nextTick();

    const rows = wrapper.findAll('nldd-list-item[data-bwb-id="BWBR0002399"]');
    expect(rows).toHaveLength(1);
    expect(rows[0]!.get('nldd-text-cell').attributes('text')).toBe('Voorbeeldwet extern');
  });

  it('wisselt met de segmented control tussen zoeken en uploaden', async () => {
    const wrapper = mount(AddLawSheet);
    // Standaard staat de zoek-tab open; de upload-knop hoort er dan niet te zijn.
    expect(wrapper.find('[data-testid="add-law-upload"]').exists()).toBe(false);
    internals(wrapper).mode = 'upload';
    await nextTick();
    expect(wrapper.find('[data-testid="add-law-upload"]').exists()).toBe(true);
  });

  it('biedt de document-upload als route en emit "upload-requested" (sheet sluit eerst)', async () => {
    // De file-picker zelf zit in LibraryView; de sheet moet vóór de emit sluiten
    // zodat de picker niet achter de sheet opent.
    const wrapper = mount(AddLawSheet);
    const hide = vi.fn();
    internals(wrapper).$refs.sheetRef.hide = hide;
    internals(wrapper).mode = 'upload';
    await nextTick();

    await wrapper.get('[data-testid="add-law-upload"]').trigger('click');

    expect(hide).toHaveBeenCalledTimes(1);
    expect(wrapper.emitted('upload-requested')).toEqual([[]]);
  });
});
