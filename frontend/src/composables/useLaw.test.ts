import { describe, it, expect, beforeEach, vi } from 'vitest';
import { flushPromises } from '@vue/test-utils';
import { useLaw } from './useLaw';
import { res, stubFetch as stubGlobalFetch } from '../test-utils/fetchStubs';
import { installTestCorpus } from '../test-utils/corpusSetup';
import type { MemoryCorpus } from '../corpus/memoryTransport';

// Reads come from the in-memory corpus; the fetch stub only sees writes.
type FetchOptions = { method?: string; headers?: Record<string, string>; body?: unknown };
interface FetchCall {
  url: string;
  opts: FetchOptions;
}

// Install a fetch stub with the call shape these tests read.
function stubFetch(impl: (url: string, opts?: FetchOptions) => Promise<unknown>) {
  stubGlobalFetch(impl);
}

// A recorded write call, failing the test outright when it is missing, so an
// assertion on its headers cannot pass because the call never happened.
function requireCall(call: FetchCall | undefined): Required<FetchCall['opts']> & { url: string } {
  if (!call?.opts.headers) throw new Error('expected a write call with headers');
  return { url: call.url, method: call.opts.method ?? '', headers: call.opts.headers, body: call.opts.body };
}

async function waitForLoaded(law: ReturnType<typeof useLaw>) {
  await vi.waitFor(() => {
    expect(law.loading.value).toBe(false);
  });
}

const TRAJECT = 'tr-12345678';

// One traject's corpus; the route stays on the global corpus, since useLaw
// reads through the traject it was given.
function corpusWith(laws: MemoryCorpus['laws'], etags: MemoryCorpus['etags'] = {}, trajectRef = TRAJECT) {
  return installTestCorpus({ '': {}, [trajectRef]: { laws, etags } });
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('useLaw optimistic concurrency', () => {
  it('captures the ETag on load and sends it back as If-Match on save', async () => {
    corpusWith({ wet_etag_chain: '$id: wet_etag_chain\nname: V1\n' }, { wet_etag_chain: '"v1"' });
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      return res({ etag: '"new"', json: { pr: null, etag: '"new"' } });
    });

    const law = useLaw('wet_etag_chain', null, TRAJECT);
    await waitForLoaded(law);
    expect(law.currentEtag.value).toBe('"v1"');

    await law.saveLaw('$id: wet_etag_chain\nname: V2\n');

    const put = calls.find((c) => c.opts?.method === 'PUT');
    expect(put).toBeTruthy();
    expect(requireCall(put).url).toBe('/api/trajects/tr-12345678/corpus/laws/wet_etag_chain');
    expect(requireCall(put).headers['If-Match']).toBe('"v1"');
    // The new ETag from the save response is chained for the next save.
    expect(law.currentEtag.value).toBe('"new"');
  });

  it('omits If-Match when the load carried no ETag (older deployments)', async () => {
    corpusWith({ wet_no_etag: '$id: wet_no_etag\nname: V1\n' });
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      return res({ json: { pr: null } });
    });

    const law = useLaw('wet_no_etag', null, TRAJECT);
    await waitForLoaded(law);
    expect(law.currentEtag.value).toBeNull();

    await law.saveLaw('$id: wet_no_etag\nname: V2\n');
    const put = calls.find((c) => c.opts?.method === 'PUT');
    expect(requireCall(put).headers['If-Match']).toBeUndefined();
  });

  it('surfaces a 412 as a Dutch conflict error instead of overwriting silently', async () => {
    corpusWith({ wet_conflict: '$id: wet_conflict\nname: V1\n' }, { wet_conflict: '"v1"' });
    stubFetch(async () => res({ ok: false, status: 412 }));

    const law = useLaw('wet_conflict', null, TRAJECT);
    await waitForLoaded(law);

    await expect(law.saveLaw('$id: wet_conflict\nname: V2\n')).rejects.toThrow(
      /door iemand anders gewijzigd/i,
    );
    expect(law.saveError.value?.message).toMatch(/herlaad/i);
    // The stale ETag is kept - the user reloads to pick up a fresh one.
    expect(law.currentEtag.value).toBe('"v1"');
  });

  // Review-modus approval (EditorView's handleLawSave) saves a task's full
  // proposed YAML instead of the article-scoped splice `currentLawYaml`
  // builds. `saveLaw` takes arbitrary YAML text and PUTs it verbatim, so no
  // second save path exists for that - this test pins the contract review
  // mode relies on: the PUT body is exactly the override text passed in
  // (here, a full two-article proposal unrelated to the loaded law's single
  // article), and If-Match still carries the currently loaded ETag.
  it('sends an arbitrary override YAML verbatim as the PUT body (review-mode full-save)', async () => {
    corpusWith(
      { wet_review: '$id: wet_review\narticles:\n  - number: "1"\n    text: origineel\n' },
      { wet_review: '"v1"' },
    );
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      return res({ etag: '"v2"', json: { pr: null, etag: '"v2"' } });
    });

    const law = useLaw('wet_review', null, TRAJECT);
    await waitForLoaded(law);
    expect(law.currentEtag.value).toBe('"v1"');

    const overrideYaml =
      '$id: wet_review\narticles:\n  - number: "1"\n    text: voorgesteld\n' +
      '  - number: "2"\n    text: nieuw artikel\n';
    await law.saveLaw(overrideYaml);

    const put = calls.find((c) => c.opts?.method === 'PUT');
    expect(put).toBeTruthy();
    expect(requireCall(put).body).toBe(overrideYaml);
    expect(requireCall(put).headers['If-Match']).toBe('"v1"');
    expect(law.currentEtag.value).toBe('"v2"');
    // The local law state converges on the full override, not a splice -
    // both articles from the proposal are present after the save.
    expect(law.articles.value.map((a) => a.number)).toEqual(['1', '2']);
  });

  it('records the saved body for the traject it went to, so the next open needs no fetch', async () => {
    const { corpus, transport } = corpusWith({ wet_saved: '$id: wet_saved\nname: V1\n' }, { wet_saved: '"v1"' });
    stubFetch(async () => res({ etag: '"v2"', json: { pr: null, etag: '"v2"' } }));
    const law = useLaw('wet_saved', null, TRAJECT);
    await waitForLoaded(law);

    await law.saveLaw('$id: wet_saved\nname: V2\n');

    const reads = transport.calls.length;
    expect(await corpus.forTraject(TRAJECT).law('wet_saved')).toMatchObject({ lawName: 'V2', etag: '"v2"' });
    expect(transport.calls.length).toBe(reads);
  });
});

describe('useLaw reads', () => {
  it('opens a rulework with a fresh read, past the cache', async () => {
    const { corpus, transport } = corpusWith({ wet_vers: '$id: wet_vers\nname: Oud\n' });
    await corpus.forTraject(TRAJECT).law('wet_vers');
    // corpusWith above created this traject's corpus and its laws.
    transport.corpora[TRAJECT]!.laws!.wet_vers = '$id: wet_vers\nname: Nieuw\n';

    const law = useLaw('wet_vers', null, TRAJECT);
    await waitForLoaded(law);

    expect(law.lawName.value).toBe('Nieuw');
  });

  it('reads a cross-traject switch through the traject it was given, not the route', async () => {
    const { transport } = installTestCorpus({
      '': {},
      'tr-alpha': { laws: { wet_x: '$id: wet_x\nname: Alpha\n' } },
      'tr-beta': { laws: { wet_x: '$id: wet_x\nname: Beta\n' } },
    }, 'tr-alpha');
    const law = useLaw('wet_x', null, 'tr-alpha');
    await waitForLoaded(law);

    // The route still names tr-alpha: the switch runs before it commits.
    await law.switchLaw('wet_x', null, 'tr-beta');

    expect(law.lawName.value).toBe('Beta');
    expect(transport.calls.at(-1)).toBe('tr-beta law wet_x');
  });

  it('a traject switch during a direct-URL load cannot poison the new traject cache', async () => {
    // The direct-URL branch records under the traject the load was issued
    // for. If a cross-traject switchLaw lands while the response body is
    // still streaming, the late record must go to the OLD traject - never
    // the new one, where it would shadow the real law.
    const { corpus, transport } = installTestCorpus({
      '': {},
      'tr-old': {},
      'tr-new': { laws: { wet_other: '$id: wet_other\nname: X\n', wet_direct: '$id: wet_direct\nname: Echt\n' } },
    });
    let releaseText: ((text: string) => void) | undefined;
    stubFetch(async () => ({
      ok: true,
      status: 200,
      headers: { get: (k: string) => (k.toLowerCase() === 'etag' ? '"d1"' : null) },
      // Body held open so the traject switch can land mid-flight.
      text: () => new Promise((resolve) => { releaseText = resolve; }),
    }));

    const law = useLaw('/direct/wet_direct.yaml', null, 'tr-old');
    await vi.waitFor(() => expect(releaseText).toBeDefined());
    // Cross-traject navigation while the direct body is still streaming.
    await law.switchLaw('wet_other', null, 'tr-new');
    // The waitFor above guarantees the stub has assigned it.
    releaseText!('$id: wet_direct\nname: Direct\n');
    await waitForLoaded(law);

    // The same id in the new traject comes from its own corpus.
    const entry = await corpus.forTraject('tr-new').law('wet_direct');
    expect(entry.lawName).toBe('Echt');
    expect(transport.calls.at(-1)).toBe('tr-new law wet_direct');
    // And the direct body landed under the old traject.
    expect((await corpus.forTraject('tr-old').law('wet_direct')).lawName).toBe('Direct');
  });
});

describe('useLaw law_create-flow (seedFromYaml + createLaw)', () => {
  it('seedFromYaml vult de editorstate zonder netwerk en wist de load-fout', async () => {
    // De load 404't (de wet bestaat nog niet in het traject).
    stubFetch(async () =>
      res({ ok: false, status: 404, body: 'Law not found' }),
    );
    const law = useLaw('nieuwe_wet', null, 'tr-12345678');
    await waitForLoaded(law);
    expect(law.error.value).toBeTruthy();

    const seeded = law.seedFromYaml(
      '$id: nieuwe_wet\narticles:\n  - number: "1"\n    text: t\n',
    );
    expect(seeded).toBe(true);
    expect(law.error.value).toBeNull();
    expect(law.law.value?.$id).toBe('nieuwe_wet');
    expect(law.selectedArticleNumber.value).toBe('1');
    expect(law.currentEtag.value).toBeNull();
  });

  it('seedFromYaml laat de foutstaat staan bij kapotte YAML', async () => {
    stubFetch(async () =>
      res({ ok: false, status: 404, body: 'Law not found' }),
    );
    const law = useLaw('nieuwe_wet2', null, 'tr-12345678');
    await waitForLoaded(law);
    expect(law.seedFromYaml('niet: [valide yaml')).toBe(false);
    expect(law.error.value).toBeTruthy();
  });

  it('createLaw POST naar het create-endpoint en ketent de ETag voor vervolg-saves', async () => {
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'POST') {
        return res({ etag: '"created"', json: { pr: null, etag: '"created"' } });
      }
      return res({ ok: false, status: 404, body: 'Law not found' });
    });
    const law = useLaw('nieuwe_wet3', null, 'tr-12345678');
    await waitForLoaded(law);
    law.seedFromYaml('$id: nieuwe_wet3\narticles:\n  - number: "1"\n    text: t\n');

    await law.createLaw('$id: nieuwe_wet3\narticles:\n  - number: "1"\n    text: t\n');

    const post = calls.find((c) => c.opts?.method === 'POST');
    expect(post).toBeTruthy();
    expect(requireCall(post).url).toBe('/api/trajects/tr-12345678/corpus/laws');
    // Geen If-Match op een create: er is nog niets om mee te racen.
    expect(requireCall(post).headers['If-Match']).toBeUndefined();
    expect(law.currentEtag.value).toBe('"created"');
    expect(law.lawId.value).toBe('nieuwe_wet3');
  });

  it('createLaw geeft de servermelding door (bijv. een 409-slugconflict)', async () => {
    stubFetch(async (url, opts) => {
      if (opts?.method === 'POST') {
        return res({ ok: false, status: 409, body: 'Er bestaat al een regelwerk met dit $id in dit traject; pas het $id in de YAML aan.' });
      }
      return res({ ok: false, status: 404, body: 'Law not found' });
    });
    const law = useLaw('nieuwe_wet4', null, 'tr-12345678');
    await waitForLoaded(law);
    await expect(law.createLaw('$id: nieuwe_wet4\narticles: []\n')).rejects.toThrow(
      /bestaat al een regelwerk/i,
    );
    expect(law.saveError.value).toBeTruthy();
  });
});

describe('useLaw clearLaw (traject-switch reset)', () => {
  it('drops the open law so lawId settles on null and the panes go empty', async () => {
    corpusWith({ wet_clear: '$id: wet_clear\nname: V1\narticles:\n  - number: "1"\n' }, { wet_clear: '"v1"' });

    const law = useLaw('wet_clear', '1', 'tr-12345678');
    await waitForLoaded(law);
    expect(law.lawId.value).toBe('wet_clear');
    expect(law.selectedArticleNumber.value).toBe('1');

    law.clearLaw();

    // The initial-route lawParam fallback is dropped too, so lawId is truly null
    // (not the stale route slug) and the editor shows its neutral empty state.
    expect(law.lawId.value).toBeNull();
    expect(law.law.value).toBeNull();
    expect(law.selectedArticle.value).toBeNull();
    expect(law.selectedArticleNumber.value).toBeNull();
    expect(law.rawYaml.value).toBe('');
    expect(law.error.value).toBeNull();
    expect(law.loading.value).toBe(false);
  });

  it('makes an in-flight load discard its writes so a late response cannot repopulate', async () => {
    corpusWith({ wet_slow: '$id: wet_slow\nname: V1\narticles:\n  - number: "1"\n' });

    const law = useLaw('wet_slow', '1', 'tr-12345678');
    // The load is still in flight (reads resolve asynchronously). Clear before it resolves.
    law.clearLaw();
    // Let the read settle; its writes must be discarded.
    await flushPromises();

    // The stale load's writes were discarded: still cleared.
    expect(law.lawId.value).toBeNull();
    expect(law.law.value).toBeNull();
  });
});

describe('useLaw lawTrajectRef (which traject the open law belongs to)', () => {
  it('exposes the initial traject and follows a cross-traject switchLaw', async () => {
    const body = '$id: wet_x\nname: V1\narticles:\n  - number: "1"\n';
    installTestCorpus({ '': {}, 'tr-alpha': { laws: { wet_x: body } }, 'tr-beta': { laws: { wet_x: body } } });

    const law = useLaw('wet_x', '1', 'tr-alpha');
    await waitForLoaded(law);
    // Starts on the traject the composable was constructed with.
    expect(law.lawTrajectRef.value).toBe('tr-alpha');

    // A cross-traject switch retargets the open law's traject. This is the
    // signal EditorView's add-tab watch uses to avoid persisting a freshly
    // loaded law under the traject the route is still leaving during a
    // back/forward navigation.
    await law.switchLaw('wet_x', '1', 'tr-beta');
    expect(law.lawTrajectRef.value).toBe('tr-beta');

    // A same-traject switch (newTrajectRef omitted) leaves it untouched.
    await law.switchLaw('wet_x', '1');
    expect(law.lawTrajectRef.value).toBe('tr-beta');
  });
});
