// De sectie "In dit traject" in het linkermenu.
//
// Die toont wat dit traject zelf heeft aangeraakt: de diff van de trajectbranch
// tegen zijn base. Bewerken en een wet hierheen halen gaan allebei via het
// writable-own schrijfpad, dus allebei staan ze erin. Wat de branch van zijn
// base erfde staat er bewust niet in: een traject op de centrale corpus zou
// anders duizenden wetten tonen die niemand hier heeft aangeraakt.
//
// Harness: test-utils/libraryViewHarness.ts, shallow-mount with every
// composable stubbed, deliberately narrow to this one surface.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount, type VueWrapper } from '@vue/test-utils';
import { nextTick, reactive } from 'vue';
import * as harness from './test-utils/libraryViewHarness';
import { installTestCorpus, type TestCorpus } from './test-utils/corpusSetup';

/** The route fields LibraryView reads; tests reassign params freely. */
interface RouteStub {
  name: string;
  params: Record<string, string>;
  query: Record<string, string>;
  fullPath: string;
}

/** A corpus-law row as the `?ids=` route returns it (the fields these tests set). */
interface LawRow {
  law_id: string;
  display_name: string;
  source_id?: string;
  source_name: string;
  source_priority: number;
}

/** The fake backend's answer book. */
interface FakeServer {
  sourceLaws: LawRow[];
  favorites: string[];
  changed: string[];
  changedFails: boolean;
  favoritesFails: boolean;
  holdFavoritesFor: { ref: string; promise: Promise<unknown>; value: string[] } | null;
  holdChanged: Promise<unknown> | null;
}

const routeState: RouteStub = reactive({
  name: 'library-traject',
  params: { trajectRef: 'traject-abcd1234' },
  query: {},
  fullPath: '/trajecten/traject-abcd1234',
});
vi.mock('vue-router', () => harness.vueRouter(() => routeState));
vi.mock('./composables/useAuth', () => harness.auth());

// The answer book each test adjusts: the traject's sources, the laws of its
// own source, the favorites and the edited laws. Favorites go through
// apiFetch; the laws and the edited laws through the in-memory corpus, which
// reads from the same answer book below.
const { server, ApiErrorStub, apiFetch, apiFetchJson } = vi.hoisted(() => {
  class ApiErrorStub extends Error {
    status: number | undefined;
    body: string;
    constructor(message: string, { status, body = '' }: { status?: number; body?: string } = {}) {
      super(message);
      this.name = 'ApiError';
      this.status = status;
      this.body = body;
    }
  }
  const server: FakeServer = {
    // De catalogus waaruit `?ids=` zijn metadata haalt.
    sourceLaws: [],
    favorites: [],
    changed: [],
    changedFails: false,
    favoritesFails: false,
    // Een belofte die de favorieten-aanroep per traject laat hangen, zodat een
    // test een traag antwoord van het vórige traject kan laten aankomen nadat
    // het nieuwe al geladen is.
    holdFavoritesFor: null,
    // Een belofte die de changed-laws-aanroep laat hangen, zodat een test kan
    // zien wat het menu toont terwijl het antwoord van het nieuwe traject nog
    // onderweg is. `null` = meteen antwoorden.
    holdChanged: null,
  };
  const answer = (url: unknown): unknown => {
    const u = String(url);
    // Favorieten zijn per plek: `/api/favorites` in Corpus juris,
    // `/api/trajects/{ref}/favorites` in een traject. Beide eindigen op
    // `/favorites`, dus dat is genoeg om ze hier te herkennen.
    if (u.endsWith('/favorites')) {
      if (server.favoritesFails) {
        throw new ApiErrorStub('Failed to load favorites: 502', { status: 502 });
      }
      return server.favorites;
    }
    return [];
  };
  return {
    server,
    ApiErrorStub,
    apiFetch: vi.fn(async (url: unknown, _init?: unknown) => ({
      ok: true,
      status: 200,
      json: async () => answer(url),
    })),
    apiFetchJson: vi.fn(async (url: unknown, _init?: unknown) => {
      if (server.holdFavoritesFor && String(url).includes(server.holdFavoritesFor.ref)) {
        const held = server.holdFavoritesFor;
        await held.promise;
        // Zijn eigen payload, vastgelegd bij het opzetten van de hold: zou hij
        // `server.favorites` lezen dan kreeg hij de waarde van ná de wissel en
        // lokt de test de race niet uit.
        return held.value;
      }
      return answer(url);
    }),
  };
});

vi.mock('./lib/apiFetch', () =>
  harness.apiFetchModule({
    apiFetch: (...a: Parameters<typeof apiFetch>) => apiFetch(...a),
    apiFetchJson: (...a: Parameters<typeof apiFetchJson>) => apiFetchJson(...a),
    ApiError: ApiErrorStub,
  }),
);
vi.mock('./composables/useDocumentsManager', () => harness.documentsManager());
vi.mock('./composables/useTrajectDocumentJobs', () => harness.trajectDocumentJobs());
vi.mock('./composables/useDocumentUpload', () => harness.documentUpload());
vi.mock('./composables/useDocumentTaskReview', () => harness.documentTaskReview());

import LibraryView from './LibraryView.vue';

const OWN_SOURCE = 'traject-eigen';

/// Zeven wetten, met weergavenamen die bewust NIET in law_id-volgorde staan:
/// de backend sorteert op law_id, de gebruiker leest de weergavenaam.
const SEVEN_LAWS: LawRow[] = ([
  ['wet_a', 'Zorgverzekeringswet'],
  ['wet_b', 'Algemene wet bestuursrecht'],
  ['wet_c', 'Wet langdurige zorg'],
  ['wet_d', 'Besluit langdurige zorg'],
  ['wet_e', 'Regeling langdurige zorg'],
  ['wet_f', 'Kieswet'],
  ['wet_g', 'Participatiewet'],
] as const).map(([law_id, display_name]) => ({
  law_id,
  display_name,
  source_id: OWN_SOURCE,
  source_name: 'Eigen traject-repo',
  source_priority: 0,
}));

/// `n` wetten met oplopende weergavenamen (`Wet 001` … `Wet 0nn`), zodat de
/// alfabetische volgorde gelijkloopt met de nummering en een test aan de eerste
/// en laatste regel kan zien hoeveel er zijn uitgeklapt.
function manyLaws(n: number): LawRow[] {
  return Array.from({ length: n }, (_, i) => ({
    law_id: `wet_${String(i + 1).padStart(3, '0')}`,
    display_name: `Wet ${String(i + 1).padStart(3, '0')}`,
    source_id: OWN_SOURCE,
    source_name: 'Eigen traject-repo',
    source_priority: 0,
  }));
}

// Metadata for an id lookup: the sidebar resolves favorites and touched laws
// through it, so they must come from the same catalog.
const byId = (id: string): LawRow =>
  server.sourceLaws.find((l) => l.law_id === id) || {
    law_id: id,
    display_name: id,
    source_name: 'Onbekend',
    source_priority: 0,
  };

let corpus: TestCorpus;

// Both trajects read from the answer book as it stands at the time of the read.
function installCorpus() {
  corpus = installTestCorpus({ 'traject-abcd1234': {}, 'traject-99998888': {} }, routeState.params.trajectRef ?? null);
  corpus.transport.list = async (_trajectRef, query) => {
    const laws = (query.ids ?? []).map(byId);
    return { laws, total: laws.length };
  };
  corpus.transport.changedLaws = async () => {
    if (server.holdChanged) await server.holdChanged;
    if (server.changedFails) {
      throw new ApiErrorStub('Failed to load changed-laws: 502', { status: 502 });
    }
    return server.changed;
  };
}

// A traject switch as the traject chooser makes it: the route, which the
// trajecten store reads, and the traject the corpus reads from.
function switchTraject(trajectRef: string) {
  routeState.params = { trajectRef };
  corpus.trajectRef.value = trajectRef;
}

async function mountLibrary() {
  const wrapper = shallowMount(LibraryView, { global: { stubs: { teleport: true }, plugins: [harness.pinia()] } });
  for (let i = 0; i < 6; i++) await nextTick();
  return wrapper;
}

// De weergavenamen van één sidebar-sectie, in renderorde.
function sectionLaws(wrapper: VueWrapper, key: string) {
  return wrapper
    .findAll(`nldd-list-item[data-section="${key}"]`)
    .map((item) => item.find('nldd-text-cell').attributes('text'));
}

// De sleutels van de secties met wetten, in renderorde, ontdubbeld.
function sectionOrder(wrapper: VueWrapper) {
  const seen: (string | undefined)[] = [];
  for (const item of wrapper.findAll('nldd-list-item[data-section]')) {
    const key = item.attributes('data-section');
    if (!seen.includes(key)) seen.push(key);
  }
  return seen;
}

// De uitklapknop onder "In dit traject" ('Toon alle 21' / 'Toon minder').
function expander(wrapper: VueWrapper) {
  return wrapper.find('nldd-button[data-testid="traject-laws-expander"]');
}

const TRAJECT_SECTION_TITLE = 'In dit traject';

// De koppen van de secties die echt gerenderd zijn. Bewust niet via
// `wrapper.html()`: die bevat ook de HTML-comments uit het template, dus een
// comment die de sectie bij naam noemt zou zo'n assertie laten slagen of falen
// om de verkeerde reden.
function sectionTitles(wrapper: VueWrapper) {
  return wrapper.findAll('nldd-title h4').map((h) => h.text());
}

// De lege staat onder de kop: "nog niets, en dit kun je eraan doen".
function noLawsText(wrapper: VueWrapper) {
  return wrapper.find('nldd-rich-text[data-testid="traject-no-laws"]');
}

beforeEach(() => {
  apiFetch.mockClear();
  apiFetchJson.mockClear();
  localStorage.clear();
  routeState.name = 'library-traject';
  routeState.params = { trajectRef: 'traject-abcd1234' };
  server.sourceLaws = SEVEN_LAWS;
  server.favorites = [];
  server.changed = SEVEN_LAWS.map((l) => l.law_id);
  server.changedFails = false;
  server.favoritesFails = false;
  server.holdFavoritesFor = null;
  server.holdChanged = null;
  installCorpus();
});

describe('LibraryView - wat dit traject heeft aangeraakt, in het linkermenu', () => {
  it('toont de aangeraakte wetten, alfabetisch op weergavenaam', async () => {
    const wrapper = await mountLibrary();

    expect(sectionTitles(wrapper)).toContain(TRAJECT_SECTION_TITLE);
    expect(sectionLaws(wrapper, 'traject')).toEqual([
      'Algemene wet bestuursrecht',
      'Besluit langdurige zorg',
      'Kieswet',
      'Participatiewet',
      'Regeling langdurige zorg',
      'Wet langdurige zorg',
      'Zorgverzekeringswet',
    ]);
  });

  it('laat wat de branch erfde maar niet aanraakte buiten het menu', async () => {
    // De catalogus houdt zeven wetten; de branch raakte er twee aan. De andere
    // vijf horen bij wat het traject van zijn base erfde en zijn niet zijn werk.
    server.changed = ['wet_c', 'wet_f'];
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).toEqual(['Kieswet', 'Wet langdurige zorg']);
  });

  it('zet de sectie onder Favorieten en boven Recent bekeken', async () => {
    // Recent bekeken wordt per traject bewaard; de platte sleutel is legacy en
    // wordt bij het laden juist opgeruimd.
    localStorage.setItem(
      'regelrecht-recent-laws:traject-abcd1234',
      JSON.stringify([{ law_id: 'wet_a', name: 'Zorgverzekeringswet' }]),
    );
    server.favorites = ['wet_f'];
    server.changed = ['wet_c'];
    const wrapper = await mountLibrary();

    expect(sectionOrder(wrapper)).toEqual(['favorites', 'traject', 'recent']);
  });

  it('toont de kop met een lege staat als er nog niets is aangeraakt', async () => {
    // De lege staat is de sectie in wording: dezelfde kop, met eronder wat je
    // moet doen om hem te vullen. Geen rijen dus, wel de kop.
    server.changed = [];
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).toEqual([]);
    expect(sectionTitles(wrapper)).toContain(TRAJECT_SECTION_TITLE);
    expect(noLawsText(wrapper).exists()).toBe(true);
  });

  it('houdt het trajectmenu overeind als er nog niets is aangeraakt', async () => {
    // De paginabrede lege staat vervangt de hele split-view. Sloeg die aan in
    // een traject, dan verdween met de wetten ook Instellingen, Werkdocumenten
    // en Taken, en bleef er een lege pagina over.
    server.changed = [];
    server.favorites = [];
    const wrapper = await mountLibrary();

    expect(wrapper.find('nldd-navigation-split-view').exists()).toBe(true);
    expect(noLawsText(wrapper).exists()).toBe(true);
  });

  it('zegt niet "nog niets bewerkt" als de diff niet opgehaald kon worden', async () => {
    // `fetchChangedLawIds` slikt elke fout en geeft `null`. Dat is iets anders
    // dan een lege set: bij een 500 weet de app juist niet of er iets bewerkt
    // is, en dan mag de sectie dat ook niet beweren. Hij valt dan weg.
    server.changedFails = true;
    server.favorites = ['wet_f'];
    const wrapper = await mountLibrary();

    expect(sectionTitles(wrapper)).not.toContain(TRAJECT_SECTION_TITLE);
    expect(noLawsText(wrapper).exists()).toBe(false);
    // De rest van het menu staat er nog, dus dit is geen kapotte sidebar.
    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Kieswet']);
  });

  it('laat een traag antwoord van het vorige traject de favorieten niet overschrijven', async () => {
    // Favorieten zijn per traject, dus twee antwoorden kunnen het oneens zijn.
    // Landt dat van het vórige traject later, dan mag het het huidige niet
    // omverduwen: de toewijzing hoort achter de staleness-check van loadIndex.
    let releaseOld: ((value?: unknown) => void) | undefined;
    server.holdFavoritesFor = {
      ref: 'traject-abcd1234',
      promise: new Promise((resolve) => { releaseOld = resolve; }),
      value: ['wet_a'],
    };
    server.favorites = ['wet_a'];
    const wrapper = await mountLibrary();

    // Wissel naar het volgende traject; dat antwoordt meteen, met een andere set.
    // Beide wetten zitten in de catalogus, dus als het oude antwoord wint is dat
    // zichtbaar als "Zorgverzekeringswet" in plaats van weggefilterd.
    server.holdFavoritesFor = null;
    server.favorites = ['wet_a', 'wet_f'];
    switchTraject('traject-99998888');
    for (let i = 0; i < 8; i++) await nextTick();

    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Zorgverzekeringswet', 'Kieswet']);

    // En nu komt het trage antwoord van het eerste traject alsnog binnen, met
    // alleen wet_a erin. Zonder de staleness-check zou de sectie daarop
    // terugvallen en Kieswet kwijtraken.
    // The Promise executor ran synchronously when the hold was set up.
    releaseOld!();
    for (let i = 0; i < 8; i++) await nextTick();

    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Zorgverzekeringswet', 'Kieswet']);
  });

  it('toont de lege staat ook naast een favoriet', async () => {
    // De kop hoort bij deze sectie, niet bij "het menu is leeg": dat je iets
    // bewaard of bekeken hebt zegt niets over wat dit traject heeft gedaan.
    server.changed = [];
    server.favorites = ['wet_f'];
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Kieswet']);
    expect(noLawsText(wrapper).exists()).toBe(true);
  });

  it('laat een aangeraakte favoriet in beide secties staan', async () => {
    // Geen ontdubbeling: een favoriet die je hier ook bewerkte hoort in allebei
    // thuis, dezelfde houding die Recent bekeken al aanneemt.
    server.favorites = ['wet_f'];
    server.changed = ['wet_f'];
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).toEqual(['Kieswet']);
    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Kieswet']);
  });

  it('toont bij precies 20 wetten alles, zonder uitklapknop', async () => {
    server.sourceLaws = manyLaws(20);
    server.changed = server.sourceLaws.map((l) => l.law_id);
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).toHaveLength(20);
    expect(expander(wrapper).exists()).toBe(false);
  });

  it('klapt boven de 20 in, en de knop klapt heen en weer', async () => {
    server.sourceLaws = manyLaws(21);
    server.changed = server.sourceLaws.map((l) => l.law_id);
    const wrapper = await mountLibrary();

    let shown = sectionLaws(wrapper, 'traject');
    expect(shown).toHaveLength(20);
    expect(shown.at(-1)).toBe('Wet 020');
    expect(expander(wrapper).attributes('text')).toBe('Toon alle 21');

    await expander(wrapper).trigger('click');
    await nextTick();
    shown = sectionLaws(wrapper, 'traject');
    expect(shown).toHaveLength(21);
    expect(shown.at(-1)).toBe('Wet 021');
    expect(expander(wrapper).attributes('text')).toBe('Toon minder');

    await expander(wrapper).trigger('click');
    await nextTick();
    expect(sectionLaws(wrapper, 'traject')).toHaveLength(20);
    expect(expander(wrapper).attributes('text')).toBe('Toon alle 21');
  });

  it('laat na een traject-wissel geen lijst van het vorige traject staan', async () => {
    // Een favoriet houdt de split-view overeind terwijl het nieuwe traject
    // laadt, zodat de assertie hieronder over een zichtbaar menu gaat en niet
    // over een paginabrede lege staat.
    server.favorites = ['wet_f'];
    const wrapper = await mountLibrary();
    expect(sectionLaws(wrapper, 'traject')).toHaveLength(7);

    // Het volgende traject heeft nog niets aangeraakt, en laat zijn
    // changed-laws-aanroep hangen. Het venster tussen de wissel en het antwoord
    // is precies waar de reset voor is: zonder die reset staan de zeven wetten
    // van het vorige traject hier nog in het menu van het nieuwe.
    let releaseChanged: ((value?: unknown) => void) | undefined;
    server.holdChanged = new Promise((resolve) => {
      releaseChanged = resolve;
    });
    server.changed = [];
    switchTraject('traject-99998888');
    for (let i = 0; i < 4; i++) await nextTick();

    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Kieswet']);
    expect(sectionLaws(wrapper, 'traject')).toEqual([]);

    // En na het antwoord staat er de lege staat: het nieuwe traject raakte niets
    // aan. De favoriet ernaast doet daar niets aan af, want die zegt niets over
    // wat dit traject heeft gedaan.
    // The Promise executor ran synchronously when the hold was set up.
    releaseChanged!();
    for (let i = 0; i < 8; i++) await nextTick();

    expect(sectionLaws(wrapper, 'traject')).toEqual([]);
    expect(noLawsText(wrapper).exists()).toBe(true);
  });
});

describe('LibraryView - Recent bekeken naast "In dit traject"', () => {
  const RECENT_KEY = 'regelrecht-recent-laws:traject-abcd1234';
  // Stored with its name, as a visit does: a law outside the index (neither a
  // favorite nor touched by the traject) shows under that stored name.
  const setRecent = (ids: string[]) =>
    localStorage.setItem(
      RECENT_KEY,
      JSON.stringify(ids.map((law_id) => ({ law_id, name: byId(law_id).display_name }))),
    );

  it('laat een wet die in het traject staat weg uit Recent bekeken', async () => {
    setRecent(['wet_a', 'wet_f']);
    server.changed = ['wet_a'];
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).toEqual(['Zorgverzekeringswet']);
    expect(sectionLaws(wrapper, 'recent')).toEqual(['Kieswet']);
  });

  it('laat de sectie weg als alles wat bekeken is in het traject staat', async () => {
    setRecent(['wet_a']);
    server.changed = ['wet_a'];
    const wrapper = await mountLibrary();

    expect(sectionTitles(wrapper)).not.toContain('Recent bekeken');
  });

  it('laat ook een wet weg die in het traject achter de uitklapknop staat', async () => {
    server.sourceLaws = manyLaws(21);
    server.changed = server.sourceLaws.map((l) => l.law_id);
    setRecent(['wet_021']);
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).not.toContain('Wet 021');
    expect(sectionLaws(wrapper, 'recent')).toEqual([]);
  });

  it('houdt een favoriet wel in Recent bekeken', async () => {
    setRecent(['wet_f']);
    server.favorites = ['wet_f'];
    server.changed = ['wet_a'];
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Kieswet']);
    expect(sectionLaws(wrapper, 'recent')).toEqual(['Kieswet']);
  });

  it('bewaart de wet in de geschiedenis, zodat hij terugkomt als hij het traject verlaat', async () => {
    setRecent(['wet_a']);
    server.changed = ['wet_a'];
    const wrapper = await mountLibrary();
    expect(sectionLaws(wrapper, 'recent')).toEqual([]);

    expect(JSON.parse(localStorage.getItem(RECENT_KEY) || '[]')).toHaveLength(1);
    wrapper.unmount();
    server.changed = ['wet_b'];
    const again = await mountLibrary();
    expect(sectionLaws(again, 'recent')).toEqual(['Zorgverzekeringswet']);
  });
});

describe('LibraryView - snel zoeken in "In dit traject"', () => {
  const filterField = (wrapper: VueWrapper) => wrapper.find('nldd-search-field[data-testid="traject-laws-filter"]');
  const filterCount = (wrapper: VueWrapper) => wrapper.find('nldd-text[data-testid="traject-laws-filter-count"]');
  const filterEmpty = (wrapper: VueWrapper) => wrapper.find('nldd-inline-dialog[data-testid="traject-laws-filter-empty"]');
  async function type(wrapper: VueWrapper, value: string) {
    await filterField(wrapper).trigger('input', { detail: { value } });
    await nextTick();
  }
  async function mountWith(n: number) {
    server.sourceLaws = manyLaws(n);
    server.changed = server.sourceLaws.map((l) => l.law_id);
    return mountLibrary();
  }

  it('biedt het zoekveld niet aan zolang de lijst niet inklapt', async () => {
    const wrapper = await mountWith(20);
    expect(filterField(wrapper).exists()).toBe(false);
  });

  it('biedt het zoekveld aan zodra de lijst inklapt, naast de uitklapknop', async () => {
    const wrapper = await mountWith(21);
    expect(filterField(wrapper).exists()).toBe(true);
    expect(expander(wrapper).attributes('text')).toBe('Toon alle 21');
    // The live region is there from the start, empty until a search.
    expect(filterCount(wrapper).text()).toBe('');
  });

  it('zoekt in de hele lijst, ook in wat ingeklapt is', async () => {
    const wrapper = await mountWith(25);
    expect(sectionLaws(wrapper, 'traject')).not.toContain('Wet 023');

    await type(wrapper, '  wet 02 ');
    expect(sectionLaws(wrapper, 'traject')).toEqual(
      ['Wet 020', 'Wet 021', 'Wet 022', 'Wet 023', 'Wet 024', 'Wet 025'],
    );
    expect(filterCount(wrapper).text()).toBe('6 regelwerken');
  });

  it('vindt een wet ook op zijn id', async () => {
    const wrapper = await mountWith(21);
    await type(wrapper, 'wet_007');
    expect(sectionLaws(wrapper, 'traject')).toEqual(['Wet 007']);
    expect(filterCount(wrapper).text()).toBe('1 regelwerk');
  });

  it('verbergt de uitklapknop tijdens het zoeken en zet hem daarna terug', async () => {
    const wrapper = await mountWith(21);
    await type(wrapper, 'wet 00');
    expect(expander(wrapper).exists()).toBe(false);

    await type(wrapper, '');
    expect(expander(wrapper).attributes('text')).toBe('Toon alle 21');
    expect(sectionLaws(wrapper, 'traject')).toHaveLength(20);
  });

  it('houdt het zoekveld en de kop als niets past, en zegt dat', async () => {
    const wrapper = await mountWith(21);
    await type(wrapper, 'bestaat niet');

    expect(sectionLaws(wrapper, 'traject')).toEqual([]);
    expect(sectionTitles(wrapper)).toContain(TRAJECT_SECTION_TITLE);
    expect(filterField(wrapper).exists()).toBe(true);
    expect(filterEmpty(wrapper).attributes('text')).toBe('Geen regelwerken gevonden');
    expect(filterCount(wrapper).text()).toBe('0 regelwerken');
    expect(noLawsText(wrapper).exists()).toBe(false);
  });

  it('klapt de lijst uit als de gekozen treffer na het wissen achter de knop zou staan', async () => {
    const wrapper = await mountWith(25);
    await type(wrapper, 'wet 023');
    await wrapper.find('nldd-list-item[data-law-id="wet_023"]').trigger('click');
    await nextTick();

    await type(wrapper, '');
    await nextTick();
    expect(sectionLaws(wrapper, 'traject')).toHaveLength(25);
    expect(expander(wrapper).attributes('text')).toBe('Toon minder');
  });

  it('neemt de zoekterm niet mee naar een ander traject', async () => {
    const wrapper = await mountWith(21);
    await type(wrapper, 'wet 007');
    expect(sectionLaws(wrapper, 'traject')).toEqual(['Wet 007']);

    switchTraject('traject-99998888');
    for (let i = 0; i < 8; i++) await nextTick();
    expect(filterField(wrapper).attributes('value')).toBe('');
    expect(sectionLaws(wrapper, 'traject')).toHaveLength(20);
  });
});
