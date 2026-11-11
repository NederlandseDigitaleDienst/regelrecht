import { describe, it, expect, beforeEach, vi } from 'vitest';
import { flushPromises } from '@vue/test-utils';
import * as yaml from 'js-yaml';
import { useOpenLaw } from './useOpenLaw';
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

async function waitForLoaded(law: ReturnType<typeof useOpenLaw>) {
  await vi.waitFor(() => {
    expect(law.loading.value).toBe(false);
  });
}

const TRAJECT = 'tr-12345678';

// One traject's corpus; the route stays on the global corpus, since useOpenLaw
// reads through the traject it was given.
function corpusWith(laws: MemoryCorpus['laws'], etags: MemoryCorpus['etags'] = {}, trajectRef = TRAJECT) {
  return installTestCorpus({ '': {}, [trajectRef]: { laws, etags } });
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('useOpenLaw optimistic concurrency', () => {
  it('captures the ETag on load and sends it back as If-Match on save', async () => {
    corpusWith({ wet_etag_chain: '$id: wet_etag_chain\nname: V1\n' }, { wet_etag_chain: '"v1"' });
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      return res({ etag: '"new"', json: { pr: null, etag: '"new"' } });
    });

    const law = useOpenLaw({ lawId: 'wet_etag_chain', trajectRef: TRAJECT });
    await waitForLoaded(law);
    expect(law.currentEtag.value).toBe('"v1"');

    await law.save({ content: '$id: wet_etag_chain\nname: V2\n' });

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

    const law = useOpenLaw({ lawId: 'wet_no_etag', trajectRef: TRAJECT });
    await waitForLoaded(law);
    expect(law.currentEtag.value).toBeNull();

    await law.save({ content: '$id: wet_no_etag\nname: V2\n' });
    const put = calls.find((c) => c.opts?.method === 'PUT');
    expect(requireCall(put).headers['If-Match']).toBeUndefined();
  });

  it('surfaces a 412 as a Dutch conflict error instead of overwriting silently', async () => {
    corpusWith({ wet_conflict: '$id: wet_conflict\nname: V1\n' }, { wet_conflict: '"v1"' });
    stubFetch(async () => res({ ok: false, status: 412 }));

    const law = useOpenLaw({ lawId: 'wet_conflict', trajectRef: TRAJECT });
    await waitForLoaded(law);

    const outcome = await law.save({ content: '$id: wet_conflict\nname: V2\n' });
    expect(outcome.status).toBe('failed');
    expect(law.saveError.value?.message).toMatch(/door iemand anders gewijzigd/i);
    expect(law.saveError.value?.message).toMatch(/herlaad/i);
    // The stale ETag is kept - the user reloads to pick up a fresh one.
    expect(law.currentEtag.value).toBe('"v1"');
  });

  // Approving a review proposal saves the task's full proposed YAML instead
  // of the article-scoped splice `currentLawYaml` builds. `save({ content })`
  // PUTs that text verbatim, so there is no second save path for it - this
  // test pins the contract review mode relies on: the PUT body is exactly the
  // content passed in (here, a two-article proposal unrelated to the loaded
  // law's single article), and If-Match still carries the loaded ETag.
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

    const law = useOpenLaw({ lawId: 'wet_review', trajectRef: TRAJECT });
    await waitForLoaded(law);
    expect(law.currentEtag.value).toBe('"v1"');

    const overrideYaml =
      '$id: wet_review\narticles:\n  - number: "1"\n    text: voorgesteld\n' +
      '  - number: "2"\n    text: nieuw artikel\n';
    await law.save({ content: overrideYaml });

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
    const law = useOpenLaw({ lawId: 'wet_saved', trajectRef: TRAJECT });
    await waitForLoaded(law);

    await law.save({ content: '$id: wet_saved\nname: V2\n' });

    const reads = transport.calls.length;
    expect(await corpus.forTraject(TRAJECT).law('wet_saved')).toMatchObject({ lawName: 'V2', etag: '"v2"' });
    expect(transport.calls.length).toBe(reads);
  });
});

describe('useOpenLaw reads', () => {
  it('opens a rulework with a fresh read, past the cache', async () => {
    const { corpus, transport } = corpusWith({ wet_vers: '$id: wet_vers\nname: Oud\n' });
    await corpus.forTraject(TRAJECT).law('wet_vers');
    // corpusWith above created this traject's corpus and its laws.
    transport.corpora[TRAJECT]!.laws!.wet_vers = '$id: wet_vers\nname: Nieuw\n';

    const law = useOpenLaw({ lawId: 'wet_vers', trajectRef: TRAJECT });
    await waitForLoaded(law);

    expect(law.lawName.value).toBe('Nieuw');
  });

  it('reads a cross-traject switch through the traject it was given, not the route', async () => {
    const { transport } = installTestCorpus({
      '': {},
      'tr-alpha': { laws: { wet_x: '$id: wet_x\nname: Alpha\n' } },
      'tr-beta': { laws: { wet_x: '$id: wet_x\nname: Beta\n' } },
    }, 'tr-alpha');
    const law = useOpenLaw({ lawId: 'wet_x', trajectRef: 'tr-alpha' });
    await waitForLoaded(law);

    // The route still names tr-alpha: the switch runs before it commits.
    await law.open({ lawId: 'wet_x', trajectRef: 'tr-beta' });

    expect(law.lawName.value).toBe('Beta');
    expect(transport.calls.at(-1)).toBe('tr-beta law wet_x');
  });

  it('a traject switch during a direct-URL load cannot poison the new traject cache', async () => {
    // The direct-URL branch records under the traject the load was issued
    // for. If a cross-traject open lands while the response body is
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

    const law = useOpenLaw({ lawId: '/direct/wet_direct.yaml', trajectRef: 'tr-old' });
    await vi.waitFor(() => expect(releaseText).toBeDefined());
    // Cross-traject navigation while the direct body is still streaming.
    await law.open({ lawId: 'wet_other', trajectRef: 'tr-new' });
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

describe('useOpenLaw law_create flow (seedFromYaml + save({ create }))', () => {
  it('seedFromYaml fills the state without a fetch and clears the load error', async () => {
    // The load answers 404: the law does not exist in the traject yet.
    stubFetch(async () =>
      res({ ok: false, status: 404, body: 'Law not found' }),
    );
    const law = useOpenLaw({ lawId: 'nieuwe_wet', trajectRef: 'tr-12345678' });
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

  it('seedFromYaml keeps the error state for broken YAML', async () => {
    stubFetch(async () =>
      res({ ok: false, status: 404, body: 'Law not found' }),
    );
    const law = useOpenLaw({ lawId: 'nieuwe_wet2', trajectRef: 'tr-12345678' });
    await waitForLoaded(law);
    expect(law.seedFromYaml('niet: [valide yaml')).toBe(false);
    expect(law.error.value).toBeTruthy();
  });

  it('create POSTs to the create endpoint and chains the ETag for later saves', async () => {
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'POST') {
        return res({ etag: '"created"', json: { pr: null, etag: '"created"' } });
      }
      return res({ ok: false, status: 404, body: 'Law not found' });
    });
    const law = useOpenLaw({ lawId: 'nieuwe_wet3', trajectRef: 'tr-12345678' });
    await waitForLoaded(law);
    law.seedFromYaml('$id: nieuwe_wet3\narticles:\n  - number: "1"\n    text: t\n');

    await law.save({ content: '$id: nieuwe_wet3\narticles:\n  - number: "1"\n    text: t\n', create: true });

    const post = calls.find((c) => c.opts?.method === 'POST');
    expect(post).toBeTruthy();
    expect(requireCall(post).url).toBe('/api/trajects/tr-12345678/corpus/laws');
    // No If-Match on a create: there is nothing to race against yet.
    expect(requireCall(post).headers['If-Match']).toBeUndefined();
    expect(law.currentEtag.value).toBe('"created"');
    expect(law.lawId.value).toBe('nieuwe_wet3');
  });

  it('create passes the server message on (a 409 id conflict, say)', async () => {
    stubFetch(async (url, opts) => {
      if (opts?.method === 'POST') {
        return res({ ok: false, status: 409, body: 'Er bestaat al een regelwerk met dit $id in dit traject; pas het $id in de YAML aan.' });
      }
      return res({ ok: false, status: 404, body: 'Law not found' });
    });
    const law = useOpenLaw({ lawId: 'nieuwe_wet4', trajectRef: 'tr-12345678' });
    await waitForLoaded(law);
    const outcome = await law.save({ content: '$id: nieuwe_wet4\narticles: []\n', create: true });
    expect(outcome.status).toBe('failed');
    expect(law.saveError.value?.message).toMatch(/bestaat al een regelwerk/i);
  });
});

describe('useOpenLaw clear (traject-switch reset)', () => {
  it('drops the open law so lawId settles on null and the panes go empty', async () => {
    corpusWith({ wet_clear: '$id: wet_clear\nname: V1\narticles:\n  - number: "1"\n' }, { wet_clear: '"v1"' });

    const law = useOpenLaw({ lawId: 'wet_clear', articleNumber: '1', trajectRef: 'tr-12345678' });
    await waitForLoaded(law);
    expect(law.lawId.value).toBe('wet_clear');
    expect(law.selectedArticleNumber.value).toBe('1');

    law.clear();

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

    const law = useOpenLaw({ lawId: 'wet_slow', articleNumber: '1', trajectRef: 'tr-12345678' });
    // The load is still in flight (reads resolve asynchronously). Clear before it resolves.
    law.clear();
    // Let the read settle; its writes must be discarded.
    await flushPromises();

    // The stale load's writes were discarded: still cleared.
    expect(law.lawId.value).toBeNull();
    expect(law.law.value).toBeNull();
  });
});

describe('useOpenLaw lawTrajectRef (which traject the open law belongs to)', () => {
  it('exposes the initial traject and follows a cross-traject open', async () => {
    const body = '$id: wet_x\nname: V1\narticles:\n  - number: "1"\n';
    installTestCorpus({ '': {}, 'tr-alpha': { laws: { wet_x: body } }, 'tr-beta': { laws: { wet_x: body } } });

    const law = useOpenLaw({ lawId: 'wet_x', articleNumber: '1', trajectRef: 'tr-alpha' });
    await waitForLoaded(law);
    // Starts on the traject the composable was constructed with.
    expect(law.lawTrajectRef.value).toBe('tr-alpha');

    // A cross-traject switch retargets the open law's traject. This is the
    // signal EditorView's add-tab watch uses to avoid persisting a freshly
    // loaded law under the traject the route is still leaving during a
    // back/forward navigation.
    await law.open({ lawId: 'wet_x', articleNumber: '1', trajectRef: 'tr-beta' });
    expect(law.lawTrajectRef.value).toBe('tr-beta');

    // A same-traject switch (trajectRef omitted) leaves it untouched.
    await law.open({ lawId: 'wet_x', articleNumber: '1' });
    expect(law.lawTrajectRef.value).toBe('tr-beta');
  });
});

// A law with two articles, the first with a machine_readable.
const TWO_ARTICLES = [
  '$id: wet_edit',
  'articles:',
  '  - number: "1"',
  '    text: Eerste artikel',
  '    machine_readable:',
  '      definitions:',
  '        drempel: 100',
  '      execution:',
  '        actions:',
  '          - output: hoogte',
  '            value: 5',
  '  - number: "2"',
  '    text: Tweede artikel',
  '',
].join('\n');

async function openTwoArticles() {
  corpusWith({ wet_edit: TWO_ARTICLES }, { wet_edit: '"v1"' });
  const law = useOpenLaw({ lawId: 'wet_edit', articleNumber: '1', trajectRef: TRAJECT });
  await waitForLoaded(law);
  return law;
}

describe('useOpenLaw edits', () => {
  it('starts the edits from the selected article', async () => {
    const law = await openTwoArticles();
    expect(law.editedText.value).toBe('Eerste artikel');
    expect(law.machineReadable.value?.definitions).toEqual({ drempel: 100 });
    expect(law.yamlSource.value).toContain('drempel: 100');
    expect(law.articleDirty.value).toBe(false);
    expect(law.currentLawYaml.value).toBe(TWO_ARTICLES);
  });

  it('keeps the structure and the YAML text in step on a field edit', async () => {
    const law = await openTwoArticles();
    law.saveItem({ section: 'add-definition', key: 'plafond', data: 200 });
    expect(law.machineReadable.value?.definitions).toEqual({ drempel: 100, plafond: 200 });
    expect(law.yamlSource.value).toContain('plafond: 200');
    expect(law.isMachineReadableDirty.value).toBe(true);
    // The saved article is untouched until a save.
    expect(law.selectedArticle.value?.machine_readable?.definitions).toEqual({ drempel: 100 });
  });

  it('takes valid YAML from the YAML pane and keeps invalid text with its error', async () => {
    const law = await openTwoArticles();
    law.setYamlText('definitions:\n  drempel: 300\n');
    expect(law.machineReadable.value?.definitions).toEqual({ drempel: 300 });
    expect(law.parseError.value).toBeNull();

    law.setYamlText('definitions: [kapot');
    expect(law.yamlSource.value).toBe('definitions: [kapot');
    expect(law.parseError.value).toBeTruthy();
    // The last valid structure stays.
    expect(law.machineReadable.value?.definitions).toEqual({ drempel: 300 });
  });

  it('splices only the edited article into the law it would save', async () => {
    const law = await openTwoArticles();
    law.editedText.value = 'Gewijzigd';
    const saved = yaml.load(law.currentLawYaml.value ?? '') as { articles: { text: string }[] };
    expect(saved.articles.map((a) => a.text)).toEqual(['Gewijzigd', 'Tweede artikel']);
  });

  it('drops the edits when another article is selected', async () => {
    const law = await openTwoArticles();
    law.editedText.value = 'Gewijzigd';
    law.selectArticle('2');
    expect(law.editedText.value).toBe('Tweede artikel');
    expect(law.machineReadable.value).toBeNull();
    expect(law.articleDirty.value).toBe(false);
  });

  it('discards back to the saved article', async () => {
    const law = await openTwoArticles();
    law.editedText.value = 'Gewijzigd';
    law.deleteItem({ section: 'definition', key: 'drempel' });
    law.discard();
    expect(law.editedText.value).toBe('Eerste artikel');
    expect(law.machineReadable.value?.definitions).toEqual({ drempel: 100 });
    expect(law.articleDirty.value).toBe(false);
  });

  it('starts an empty machine_readable for an article without one', async () => {
    const law = await openTwoArticles();
    law.selectArticle('2');
    law.initMachineReadable();
    expect(law.machineReadable.value?.execution?.actions).toEqual([]);
    expect(law.hasMachineReadable.value).toBe(true);
  });
});

describe('useOpenLaw save', () => {
  it('writes the edited article and resets the edits to the saved version at once', async () => {
    const law = await openTwoArticles();
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      return res({ etag: '"v2"', json: { pr: null, etag: '"v2"' } });
    });
    law.editedText.value = 'Gewijzigd';

    const outcome = await law.save();

    expect(outcome).toEqual({ status: 'saved', touchedText: true });
    expect(requireCall(calls[0]).body).toContain('Gewijzigd');
    expect(law.selectedArticle.value?.text).toBe('Gewijzigd');
    expect(law.articleDirty.value).toBe(false);
    expect(law.lastSaveTouchedText.value).toBe(false);
  });

  it('keeps the edits and says which pane they came from when the save fails', async () => {
    const law = await openTwoArticles();
    stubFetch(async () => res({ ok: false, status: 412 }));
    law.saveItem({ section: 'add-definition', key: 'plafond', data: 200 });

    const outcome = await law.save();

    expect(outcome.status).toBe('failed');
    expect(law.isMachineReadableDirty.value).toBe(true);
    expect(law.lastSaveTouchedMachine.value).toBe(true);
    expect(law.lastSaveTouchedText.value).toBe(false);
  });
});

describe('useOpenLaw unsaved work (the leave guard)', () => {
  it('counts edits against the saved article', async () => {
    const law = await openTwoArticles();
    expect(law.hasUnsavedWork.value).toBe(false);
    law.editedText.value = 'Gewijzigd';
    expect(law.hasUnsavedWork.value).toBe(true);
  });

  it('does not count an untouched review proposal, but counts edits on top of it', async () => {
    const law = await openTwoArticles();
    law.seedProposal({ number: '2', text: 'Voorstel', machine_readable: null });

    expect(law.selectedArticleNumber.value).toBe('2');
    expect(law.editedText.value).toBe('Voorstel');
    // The changes bar still sees a change against the saved article...
    expect(law.articleDirty.value).toBe(true);
    // ...but leaving loses nothing: the review seeds it again on return.
    expect(law.hasUnsavedWork.value).toBe(false);

    law.editedText.value = 'Voorstel, aangepast';
    expect(law.hasUnsavedWork.value).toBe(true);
  });
});

describe('useOpenLaw action edits', () => {
  // The action as the Machine pane hands it over: an element of the edits.
  function firstAction(law: Awaited<ReturnType<typeof openTwoArticles>>) {
    const action = law.machineReadable.value?.execution?.actions?.[0];
    if (!action) throw new Error('expected an action');
    return action;
  }

  it('edits a copy, so the article only changes on commit', async () => {
    const law = await openTwoArticles();
    law.beginActionEdit(firstAction(law));
    const draft = law.activeAction.value;
    if (!draft) throw new Error('expected an action edit');

    draft.value = 7;
    expect(firstAction(law).value).toBe(5);
    expect(law.hasUnsavedWork.value).toBe(true);

    expect(law.commitActionEdit()).toBeNull();
    expect(firstAction(law).value).toBe(7);
    expect(law.yamlSource.value).toContain('value: 7');
    expect(law.activeAction.value).toBeNull();
  });

  it('leaves the article as it was on cancel', async () => {
    const law = await openTwoArticles();
    law.beginActionEdit(firstAction(law));
    if (law.activeAction.value) law.activeAction.value.value = 7;

    law.cancelActionEdit();

    expect(firstAction(law).value).toBe(5);
    expect(law.hasUnsavedWork.value).toBe(false);
  });

  it('adds a new action only once it is committed, and refuses an unfilled stub', async () => {
    const law = await openTwoArticles();
    law.beginNewAction();
    expect(law.activeActionIsNew.value).toBe(true);
    expect(law.machineReadable.value?.execution?.actions).toHaveLength(1);
    // An untouched new action is no work to lose yet.
    expect(law.hasUnsavedWork.value).toBe(false);

    const draft = law.activeAction.value;
    if (!draft) throw new Error('expected an action edit');
    draft.output = 'nieuw';
    expect(law.commitActionEdit()).toBe("Operatie 'EQUALS' is nog niet ingevuld");
    expect(law.parseError.value).toBe("Operatie 'EQUALS' is nog niet ingevuld");

    draft.value = { operation: 'EQUALS', subject: '$a', value: 1 };
    expect(law.commitActionEdit()).toBeNull();
    expect(law.machineReadable.value?.execution?.actions?.map((a) => a.output)).toEqual(['hoogte', 'nieuw']);
  });

  it('refuses an action without an output name', async () => {
    const law = await openTwoArticles();
    law.beginActionEdit(firstAction(law));
    if (law.activeAction.value) law.activeAction.value.output = ' ';
    expect(law.commitActionEdit()).toBe('Output mag niet leeg zijn');
    expect(law.activeAction.value).not.toBeNull();
  });

  it('drops an open action edit when another article is selected', async () => {
    const law = await openTwoArticles();
    law.beginActionEdit(firstAction(law));
    law.selectArticle('2');
    expect(law.activeAction.value).toBeNull();
  });
});
