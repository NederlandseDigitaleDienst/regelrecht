// Review mode on its own: the real useOpenLaw over an in-memory corpus, a
// route stub that navigates the way the editor's route guard does, and a
// stubbed task API. EditorView.taskReview.test.ts covers how the view wires
// this module in.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { effectScope, nextTick, type EffectScope } from 'vue';
import { flushPromises } from '@vue/test-utils';
import { useOpenLaw, type OpenLaw } from './useOpenLaw';
import { useProposalReview } from './useProposalReview';
import { useEnrichVerdictsStore } from '../stores/enrichVerdicts';
import { installTestCorpus } from '../test-utils/corpusSetup';
import { res, stubFetch } from '../test-utils/fetchStubs';

// The route the module reads `?task=` from. Reactive, as a real route is.
const route = await vi.hoisted(async () => {
  const { reactive } = await import('vue');
  return reactive({ query: {} as Record<string, string> });
});
interface EditorTarget {
  name: string;
  params: { lawId?: string | null; articleNumber?: string | null };
  query?: Record<string, string>;
}
// What router.replace did, as the editor would: the URL changes, and the
// route guard selects the article it names.
const replace = vi.fn();
let openLaw: OpenLaw | null = null;
vi.mock('vue-router', () => ({
  useRoute: () => route,
  useRouter: () => ({
    replace: (to: EditorTarget) => {
      replace(to);
      route.query = { ...(to.query ?? {}) };
      const article = to.params?.articleNumber;
      if (article && openLaw) openLaw.selectArticle(article);
    },
  }),
}));

const tasks = vi.hoisted(() => ({
  fetchTask: vi.fn(),
  resolveTask: vi.fn(),
  fetchJobTasks: vi.fn(),
  applyEnrichment: vi.fn(),
}));
vi.mock('./useTasks', () => ({ useTaskActions: () => tasks }));

const TRAJECT = 'tr-12345678';
const JOB = 'job-1';
const LAW_PATH = 'corpus/regulation/nl/wet/test_wet/2025-01-01.yaml';
const law = (one: string, two: string) =>
  ['$id: test_wet', 'articles:', "  - number: '1'", `    text: ${one}`, "  - number: '2'", `    text: ${two}`, ''].join('\n');
const SAVED = law('een', 'twee');
const PROPOSAL = law('een, voorgesteld', 'twee, voorgesteld');
// Another law with the same article numbers: an article number alone does not
// say which law a task is about.
const OTHER_LAW = ['$id: andere_wet', 'articles:', "  - number: '1'", '    text: ander', ''].join('\n');

function openTask(overrides: Record<string, unknown> = {}) {
  return {
    id: 't1',
    task_type: 'job_review',
    status: 'open',
    job_id: JOB,
    title: 'Verrijking beoordelen: test_wet artikel 1',
    payload: { law_id: 'test_wet', article: '1', yaml_path: LAW_PATH },
    results: [{ path: LAW_PATH, content: PROPOSAL }],
    ...overrides,
  };
}
const secondTask = () => openTask({ id: 't2', payload: { law_id: 'test_wet', article: '2', yaml_path: LAW_PATH } });
const twoParts = () => ({
  tasks: [
    { id: 't1', article: '1', status: 'open', title: 'artikel 1' },
    { id: 't2', article: '2', status: 'open', title: 'artikel 2' },
  ],
});

// An editor route as EditorView builds it: it carries the task unless told not to.
function routeFor(lawId: string | null | undefined, articleNumber: string | null | undefined, { keepReviewTask = true } = {}) {
  const task = keepReviewTask && lawId ? route.query.task : undefined;
  return { name: 'editor-traject', params: { lawId, articleNumber }, ...(task ? { query: { task } } : {}) };
}

const discard = vi.fn();
const reloadNotes = vi.fn();
let scope: EffectScope | null = null;

async function settle() {
  for (let i = 0; i < 4; i++) {
    await flushPromises();
    await nextTick();
  }
}

/** Open `lawId`/`article` with `?task=` set, as a link from the task list does. */
async function startReview({ task = 't1' as string | null, lawId = 'test_wet', article = '1' } = {}) {
  route.query = task ? { task } : {};
  scope = effectScope();
  const made = scope.run(() => {
    const opened = useOpenLaw({ lawId, articleNumber: article, trajectRef: TRAJECT });
    openLaw = opened;
    const review = useProposalReview({
      openLaw: opened,
      routeFor,
      leaveRoute: () => ({ name: 'home' }),
      discard: () => {
        discard();
        opened.discard();
      },
      reloadNotes,
    });
    return { law: opened, review };
  });
  if (!made) throw new Error('effect scope did not run');
  await settle();
  return made;
}

beforeEach(() => {
  for (const fn of Object.values(tasks)) fn.mockReset();
  tasks.fetchTask.mockImplementation(async (id: string) => (id === 't2' ? secondTask() : openTask()));
  tasks.fetchJobTasks.mockResolvedValue(twoParts());
  tasks.applyEnrichment.mockResolvedValue({ accepted: 1, total: 2 });
  replace.mockReset();
  discard.mockReset();
  reloadNotes.mockReset();
  window.sessionStorage.clear();
  installTestCorpus(
    { '': {}, [TRAJECT]: { laws: { test_wet: SAVED, andere_wet: OTHER_LAW }, etags: { test_wet: '"v1"' } } },
    TRAJECT,
  );
});

afterEach(() => {
  scope?.stop();
  scope = null;
  openLaw = null;
});

describe('useProposalReview: the task', () => {
  it('puts the proposal in the article it is about, and says where you are in the enrichment', async () => {
    const { law, review } = await startReview();

    expect(review.active.value).toBe(true);
    expect(law.editedText.value).toBe('een, voorgesteld');
    expect(law.selectedArticle.value?.text).toBe('een');
    expect(review.statusText.value).toContain('Onderdeel 1 van 2');
    expect(review.canRejectRest.value).toBe(true);
  });

  it('treats the task as the whole enrichment when its parts cannot be fetched', async () => {
    tasks.fetchJobTasks.mockRejectedValue(new Error('netwerk'));
    const { review } = await startReview();

    expect(review.active.value).toBe(true);
    expect(review.statusText.value).not.toContain('Onderdeel');
    expect(review.canRejectRest.value).toBe(false);
  });

  it.each([
    ['a handled task', openTask({ status: 'done' }), 'Deze taak is al afgehandeld.'],
    ['a task of another type', openTask({ task_type: 'document_review' }), 'Deze taak is al afgehandeld.'],
    ['a task without a law result', openTask({ results: [] }), 'Geen resultaat gevonden bij deze taak.'],
    ['a task without a law id', openTask({ payload: { article: '1' } }), 'Geen resultaat gevonden bij deze taak.'],
    ['a task without a payload', openTask({ payload: undefined }), 'Geen resultaat gevonden bij deze taak.'],
  ])('refuses %s and says why', async (_label, task, message) => {
    tasks.fetchTask.mockResolvedValue(task);
    const { law, review } = await startReview();

    expect(review.active.value).toBe(false);
    expect(review.loadError.value).toBe(message);
    expect(review.statusText.value).toBe(message);
    expect(review.variant.value).toBe('critical');
    expect(law.editedText.value).toBe('een');
  });

  it('says so when the task cannot be fetched', async () => {
    tasks.fetchTask.mockRejectedValue(new Error('netwerk'));
    const { review } = await startReview();
    expect(review.loadError.value).toBe('Taak laden mislukt.');
  });

  it('takes the law file at payload.yaml_path, not the first file of the result', async () => {
    tasks.fetchTask.mockResolvedValue(
      openTask({ results: [{ path: 'features/zorg.feature', content: 'Feature: x' }, { path: LAW_PATH, content: PROPOSAL }] }),
    );
    const { law } = await startReview();
    expect(law.editedText.value).toBe('een, voorgesteld');
  });

  it('stays where you entered when that is another article, and seeds once you go to the task', async () => {
    const { law, review } = await startReview({ article: '2' });
    expect(law.selectedArticleNumber.value).toBe('2');
    expect(review.active.value).toBe(false);

    law.selectArticle('1');
    await settle();
    expect(review.active.value).toBe(true);
    expect(law.editedText.value).toBe('een, voorgesteld');
  });

  it('does not seed into another law with the same article number', async () => {
    const { law, review } = await startReview({ lawId: 'andere_wet' });
    expect(review.active.value).toBe(false);
    expect(law.editedText.value).toBe('ander');
  });

  it('seeds again when you come back after a detour, without fetching the task again', async () => {
    const { law, review } = await startReview();
    law.selectArticle('2');
    await settle();
    expect(review.active.value).toBe(false);
    expect(law.editedText.value).toBe('twee');

    law.selectArticle('1');
    await settle();
    expect(review.active.value).toBe(true);
    expect(law.editedText.value).toBe('een, voorgesteld');
    expect(tasks.fetchTask).toHaveBeenCalledTimes(1);
  });

  it('ends when ?task= leaves the URL, and loads the task again when it comes back', async () => {
    const { review } = await startReview();
    route.query = {};
    await settle();
    expect(review.active.value).toBe(false);
    expect(review.statusText.value).not.toContain('Onderdeel');

    route.query = { task: 't1' };
    await settle();
    expect(review.active.value).toBe(true);
    expect(tasks.fetchTask).toHaveBeenCalledTimes(2);
  });

  it('drops the error of a failed task when the URL moves on to another one', async () => {
    tasks.fetchTask.mockRejectedValueOnce(new Error('netwerk'));
    const { review } = await startReview();
    expect(review.loadError.value).toBe('Taak laden mislukt.');

    route.query = { task: 't1-opnieuw' };
    await settle();
    expect(review.loadError.value).toBeNull();
    expect(review.active.value).toBe(true);
  });
});

describe('useProposalReview: deciding', () => {
  it('records a verdict on one part, writes nothing, and moves on to the next part', async () => {
    const { law, review } = await startReview();
    await review.approve();
    await settle();

    expect(tasks.applyEnrichment).not.toHaveBeenCalled();
    expect(replace).toHaveBeenLastCalledWith(expect.objectContaining({ query: { task: 't2' } }));
    expect(law.selectedArticleNumber.value).toBe('2');
    expect(law.editedText.value).toBe('twee, voorgesteld');
    expect(review.statusText.value).toContain('Dit is het laatste onderdeel');
  });

  it('writes the enrichment in one go after the last verdict, then reloads and ends the review', async () => {
    const { review } = await startReview();
    await review.approve();
    await settle();
    await review.reject();
    await settle();

    expect(tasks.applyEnrichment).toHaveBeenCalledWith(
      JOB,
      [
        { task_id: 't1', action: 'approved', content: expect.stringContaining('een, voorgesteld') },
        { task_id: 't2', action: 'rejected' },
      ],
      '"v1"',
    );
    expect(reloadNotes).toHaveBeenCalled();
    expect(route.query.task).toBeUndefined();
    expect(review.active.value).toBe(false);
    expect(useEnrichVerdictsStore().forJob(JOB)).toEqual({});
  });

  it('finishes with every undecided part not taken', async () => {
    const { review } = await startReview();
    await review.rejectRest();

    expect(tasks.applyEnrichment).toHaveBeenCalledWith(
      JOB,
      [
        { task_id: 't1', action: 'rejected' },
        { task_id: 't2', action: 'rejected' },
      ],
      '"v1"',
    );
  });

  it('sends only the parts that are still open', async () => {
    tasks.fetchJobTasks.mockResolvedValue({
      tasks: [
        { id: 't1', article: '1', status: 'open', title: 'artikel 1' },
        { id: 't2', article: '2', status: 'done', title: 'artikel 2' },
      ],
    });
    const { review } = await startReview();
    await review.reject();

    expect(tasks.applyEnrichment).toHaveBeenCalledWith(JOB, [{ task_id: 't1', action: 'rejected' }], '"v1"');
  });

  it('keeps the review and its verdicts when writing fails', async () => {
    tasks.applyEnrichment.mockRejectedValue(new Error('412'));
    const { review } = await startReview();
    await review.approve();
    await settle();
    await review.approve();
    await settle();

    expect(review.processError.value?.message).toBe('412');
    expect(review.active.value).toBe(true);
    expect(Object.keys(useEnrichVerdictsStore().forJob(JOB))).toEqual(['t1', 't2']);
  });

  it('refuses to approve a task whose enrichment is gone, and keeps its verdict', async () => {
    tasks.fetchTask.mockResolvedValue(openTask({ job_id: null }));
    const { review } = await startReview();
    await review.approve();

    expect(review.processError.value?.message).toContain('hoort bij geen verrijking meer');
    expect(tasks.resolveTask).not.toHaveBeenCalled();
    expect(tasks.applyEnrichment).not.toHaveBeenCalled();
    expect(useEnrichVerdictsStore().forJob('t1')).toHaveProperty('t1');
  });

  it('releases a task whose enrichment is gone when it is rejected', async () => {
    tasks.fetchTask.mockResolvedValue(openTask({ job_id: null }));
    const { review } = await startReview();
    await review.reject();

    expect(tasks.resolveTask).toHaveBeenCalledWith('t1', 'rejected');
    expect(tasks.applyEnrichment).not.toHaveBeenCalled();
  });

  it('does nothing when asked to decide after the review has ended', async () => {
    const { law, review } = await startReview();
    route.query = {};
    await settle();
    law.editedText.value = 'een onopgeslagen wijziging';
    replace.mockReset();

    await review.approve();
    await review.reject();
    await review.rejectRest();

    expect(replace).not.toHaveBeenCalled();
    expect(tasks.applyEnrichment).not.toHaveBeenCalled();
    expect(tasks.resolveTask).not.toHaveBeenCalled();
    expect(law.editedText.value).toBe('een onopgeslagen wijziging');
  });
});

describe('useProposalReview: a new regelwerk from a document conversion', () => {
  const lawCreateTask = () =>
    openTask({
      job_id: null,
      payload: { law_id: 'nieuwe_wet', kind: 'law_create', yaml_path: 'nieuwe_wet.yaml' },
      results: [{ path: 'nieuwe_wet.yaml', content: "$id: nieuwe_wet\narticles:\n  - number: '1'\n    text: nieuw\n" }],
    });

  it('shows the proposed regelwerk where the law is not found yet', async () => {
    tasks.fetchTask.mockResolvedValue(lawCreateTask());
    const { law, review } = await startReview({ lawId: 'nieuwe_wet' });

    expect(review.active.value).toBe(true);
    expect(review.isLawCreate.value).toBe(true);
    expect(law.selectedArticle.value?.text).toBe('nieuw');
    expect(review.statusText.value).toContain('Dit is een nieuw regelwerk uit documentconversie');
    expect(review.variant.value).toBe('accent');
  });

  it('creates the regelwerk on approve, resolves the task and ends the review', async () => {
    tasks.fetchTask.mockResolvedValue(lawCreateTask());
    const calls: { url: string; method?: string }[] = [];
    stubFetch(async (url: string, opts?: { method?: string }) => {
      calls.push({ url, method: opts?.method });
      return res({ etag: '"c1"', json: { pr: null, etag: '"c1"' } });
    });
    const { review } = await startReview({ lawId: 'nieuwe_wet' });
    await review.approve();
    await settle();

    expect(calls).toEqual([{ url: `/api/trajects/${TRAJECT}/corpus/laws`, method: 'POST' }]);
    expect(tasks.resolveTask).toHaveBeenCalledWith('t1', 'approved');
    expect(route.query.task).toBeUndefined();
  });

  it('rejects without creating anything, and leaves the law route', async () => {
    tasks.fetchTask.mockResolvedValue(lawCreateTask());
    const { review } = await startReview({ lawId: 'nieuwe_wet' });
    await review.reject();

    expect(tasks.resolveTask).toHaveBeenCalledWith('t1', 'rejected');
    expect(replace).toHaveBeenLastCalledWith({ name: 'home' });
  });
});
