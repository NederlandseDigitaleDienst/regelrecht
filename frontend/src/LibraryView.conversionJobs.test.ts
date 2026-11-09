// Targeted tests for the conversion-jobs watcher in LibraryView.vue: what
// happens to a job the user is viewing when it leaves the polled list.
//
// The endpoint only ever returns pending/processing jobs (failures become a
// job_failed task instead - see list_traject_document_jobs, which the editor-api
// always calls with include_failed = false since #939 dropped the
// tasks.job_review flag), so "gone from the list" means completed OR failed.
// Only a written .md tells them apart. Harness: test-utils/libraryViewHarness.ts,
// shallow-mount with every composable stubbed, deliberately narrow to this one
// surface.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount } from '@vue/test-utils';
import { ref, nextTick } from 'vue';
import type { DocumentEntry } from './composables/useTrajectDocuments';
import type { DocumentJob } from './composables/useTrajectDocumentJobs';
import * as harness from './test-utils/libraryViewHarness';

/** The job fields these tests set; the view reads no others. */
type JobStub = Pick<DocumentJob, 'id' | 'target_path' | 'status'>;
type UploadedCallback = (...args: unknown[]) => void;

const routeState = {
  name: 'werkdocumenten-traject',
  params: { trajectRef: 'traject-abcd1234', docPath: '' },
  query: {},
  fullPath: '/traject/traject-abcd1234/werkdocumenten',
};
vi.mock('vue-router', () => harness.vueRouter(() => routeState));
vi.mock('./composables/useAuth', () => harness.auth());
vi.mock('./composables/useTrajects', () => harness.trajectsModule());
vi.mock('./lib/apiFetch', () => harness.apiFetchModule());
vi.mock('./composables/useFeatureFlags', () => harness.featureFlags());

// Module-level so each test can drive the exact state the watcher reacts to.
const documents = ref<DocumentEntry[]>([]);
const openDoc = vi.fn().mockResolvedValue(undefined);
const closeDoc = vi.fn();
const refreshList = vi.fn().mockResolvedValue(undefined);
vi.mock('./composables/useDocumentsManager', () =>
  harness.documentsManager(() => ({ documents, open: openDoc, close: closeDoc, refreshList })),
);

const jobs = ref<JobStub[]>([]);
const cancelJob = vi.fn();
vi.mock('./composables/useTrajectDocumentJobs', () =>
  harness.trajectDocumentJobs(() => ({ jobs, cancelJob })),
);

// Capture the onUploaded callbacks LibraryView hands in, so a test can fire
// one with an upload result the way a real successful upload would.
// LibraryView wires useDocumentUpload twice (werkdocument-upload eerst, dan
// de wet-upload), dus per aanroep vastleggen; `onUploaded` = de werkdoc-cb.
let onUploadedCbs: (UploadedCallback | null | undefined)[] = [];
const onUploaded = (...args: unknown[]) => onUploadedCbs[0]?.(...args);
vi.mock('./composables/useDocumentUpload', () =>
  harness.documentUpload((_uploadFn: unknown, uploadedCb?: UploadedCallback | null) => {
    onUploadedCbs.push(uploadedCb);
    return {};
  }),
);

vi.mock('./composables/useDocumentTaskReview', () => harness.documentTaskReview());

import LibraryView from './LibraryView.vue';

const JOB_PATH = 'upload.md';

// Mount already viewing JOB_PATH's job: the URL addresses it and the jobs
// arrive a tick later, which is the watcher's own deep-link path into
// viewingJobPath.
async function mountViewingJob() {
  routeState.params = { trajectRef: 'traject-abcd1234', docPath: JOB_PATH };
  const wrapper = shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
  await nextTick();
  jobs.value = [{ id: 'job-1', target_path: JOB_PATH, status: 'processing' }];
  await nextTick();
  await nextTick();
  openDoc.mockClear();
  return wrapper;
}

// Drop the job from the polled list and let the (async) watcher settle.
async function jobLeavesList() {
  jobs.value = [];
  await nextTick();
  await nextTick();
  await nextTick();
}

beforeEach(() => {
  onUploadedCbs = [];
  documents.value = [];
  jobs.value = [];
  openDoc.mockReset().mockResolvedValue(undefined);
  closeDoc.mockReset();
  refreshList.mockReset().mockResolvedValue(undefined);
  cancelJob.mockReset();
  routeState.name = 'werkdocumenten-traject';
  routeState.params = { trajectRef: 'traject-abcd1234', docPath: '' };
  routeState.query = {};
});

describe('LibraryView conversion-jobs watcher', () => {
  it('opens the converted document once the job completed and its .md exists', async () => {
    await mountViewingJob();
    // A completed conversion leaves the list AND writes its document.
    documents.value = [{ path: JOB_PATH }];
    await jobLeavesList();
    expect(openDoc).toHaveBeenCalledWith(JOB_PATH);
  });

  it('does not open a document for a job that left the list without writing one', async () => {
    await mountViewingJob();
    // A failed conversion leaves the list too (the endpoint excludes failed
    // rows) but never wrote its .md. Opening it would 404 into a generic load
    // error instead of reporting the failure.
    await jobLeavesList();
    expect(openDoc).not.toHaveBeenCalled();
  });

  it('reports the failure in the job view when the job vanished without its .md', async () => {
    const wrapper = await mountViewingJob();
    await jobLeavesList();
    // Match the dialog's own text, not a bare 'mislukt' - "Uploaden mislukt"
    // also renders in this view, which makes a loose assertion pass regardless.
    expect(wrapper.html()).toContain('Conversie mislukt');
  });

  it('selects a fresh upload so the main pane shows its conversion', async () => {
    const wrapper = shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
    await nextTick();
    // Only the upload response knows where the conversion will land.
    onUploaded({ ok: true, targetPath: JOB_PATH });
    jobs.value = [{ id: 'job-1', target_path: JOB_PATH, status: 'processing' }];
    await nextTick();
    await nextTick();
    expect(wrapper.html()).toContain('Aan het converteren');
    expect(openDoc).not.toHaveBeenCalled(); // no .md exists yet
  });

  it('names the document in the toolbar, not in the dialog', async () => {
    const wrapper = await mountViewingJob();
    // The title belongs where a document's title lives, so the dialog can stay
    // about the conversion. It is there from the start (the path is known) and
    // survives the job resolving either way.
    expect(wrapper.html()).toContain('<nldd-toolbar-title slot="center" align="center" text="upload"');
    await jobLeavesList();
    expect(wrapper.html()).toContain('<nldd-toolbar-title slot="center" align="center" text="upload"');
  });

  it('offers a way back out of the job view', async () => {
    // On a stacked (small) viewport the job view is the whole screen, with no
    // document list beside it to click back to. Whether the item is *visible*
    // is the pane's call via --context-back-button-display; that it exists at
    // all is this view's.
    const wrapper = await mountViewingJob();
    expect(wrapper.html()).toContain('Terug naar werkdocumenten');
  });

  it('keeps the job view while the job is still listed', async () => {
    await mountViewingJob();
    jobs.value = [{ id: 'job-1', target_path: JOB_PATH, status: 'failed' }];
    await nextTick();
    await nextTick();
    expect(openDoc).not.toHaveBeenCalled();
  });
});
