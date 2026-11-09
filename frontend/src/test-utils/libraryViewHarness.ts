// Shared stubs for the LibraryView.*.test.ts files. Each of those files
// shallow-mounts LibraryView with every composable it touches replaced by a
// controllable stub, deliberately narrow to the one surface it tests.
//
// `vi.mock` factories are hoisted above the test file's module-level consts,
// so a file cannot hand its own refs and spies to a factory directly. Every
// helper here therefore takes `overrides` as a function that runs when
// LibraryView calls the composable (at mount, when those consts exist) and
// receives the composable's arguments; what it returns wins over the default.
// The exception is `apiFetchModule`: it takes its values eagerly, when the
// factory runs, so values passed there have to come from `vi.hoisted`.
//
// Usage, with the import above `import LibraryView`:
//
//   vi.mock('vue-router', () => harness.vueRouter(() => routeState));
//   vi.mock('./composables/useDocumentsManager', () =>
//     harness.documentsManager(() => ({ open: openDoc })));
import { vi } from 'vitest';
import { ref } from 'vue';

/**
 * Fields that replace a stub's defaults, computed when LibraryView calls the
 * composable. `A` is the composable's argument list, so an override that reads
 * an argument can type it.
 */
export type Overrides<A extends unknown[] = unknown[]> = (...args: A) => Record<string, unknown>;

const apply = <A extends unknown[]>(overrides: Overrides<A> | undefined, args: A) =>
  overrides ? overrides(...args) : {};

export function vueRouter(route: () => unknown, router?: Overrides<[]>) {
  return {
    useRoute: () => route(),
    useRouter: () => ({
      replace: vi.fn().mockResolvedValue(undefined),
      push: vi.fn(),
      resolve: () => ({ href: '#' }),
      ...apply(router, []),
    }),
    onBeforeRouteUpdate: vi.fn(),
    onBeforeRouteLeave: vi.fn(),
  };
}

export function auth() {
  return { useAuth: () => ({ authenticated: ref(true), login: vi.fn() }) };
}

export function trajectsModule<A extends unknown[]>(overrides?: Overrides<A>) {
  return {
    useTrajects: (...args: A) => ({
      activeTrajectRef: ref<string | null>('traject-abcd1234'),
      activeTraject: ref({ name: 'Testtraject' }),
      ...apply(overrides, args),
    }),
    refreshTrajects: vi.fn(),
  };
}

export function apiFetchModule(module: Record<string, unknown> = {}) {
  return {
    apiFetch: vi.fn().mockResolvedValue({ ok: true, status: 200, json: async () => [] }),
    apiFetchJson: vi.fn().mockResolvedValue([]),
    ApiError: class ApiError extends Error {},
    ...module,
  };
}

export function featureFlags() {
  return { useFeatureFlags: () => ({ isEnabled: () => true }) };
}

export function documentsManager<A extends unknown[]>(overrides?: Overrides<A>) {
  return {
    useDocumentsManager: (...args: A) => ({
      documents: ref([]),
      listLoading: ref(false),
      listError: ref(null),
      currentPath: ref<string | null>(null),
      currentBody: ref(''),
      hasChanges: ref(false),
      docLoading: ref(false),
      docError: ref(null),
      saving: ref(false),
      open: vi.fn().mockResolvedValue(undefined),
      startNew: vi.fn(),
      close: vi.fn(),
      uploadDocument: vi.fn(),
      displayTitle: (p: string | null | undefined) => String(p ?? '').replace(/\.md$/, ''),
      dropDraft: vi.fn(),
      refreshList: vi.fn().mockResolvedValue(undefined),
      ...apply(overrides, args),
    }),
  };
}

export function trajectDocumentJobs<A extends unknown[]>(overrides?: Overrides<A>) {
  return {
    useTrajectDocumentJobs: (...args: A) => ({
      jobs: ref([]),
      cancelJob: vi.fn(),
      refresh: vi.fn(),
      startPolling: vi.fn(),
      stopPolling: vi.fn(),
      ...apply(overrides, args),
    }),
  };
}

export function documentUpload<A extends unknown[]>(overrides?: Overrides<A>) {
  return {
    useDocumentUpload: (...args: A) => ({
      fileInput: ref(null),
      uploadError: ref(null),
      uploadRetryable: ref(false),
      pendingFile: ref(null),
      onUpload: vi.fn(),
      onFileChange: vi.fn(),
      confirmUpload: vi.fn(),
      cancelUpload: vi.fn(),
      ...apply(overrides, args),
    }),
  };
}

export function documentTaskReview<A extends unknown[]>(overrides?: Overrides<A>) {
  return {
    useDocumentTaskReview: (...args: A) => ({
      reviewTask: ref(null),
      proposedContent: ref(null),
      loadError: ref(null),
      loadReview: vi.fn().mockResolvedValue(undefined),
      approveAfterSave: vi.fn(),
      reject: vi.fn(),
      ...apply(overrides, args),
    }),
  };
}
