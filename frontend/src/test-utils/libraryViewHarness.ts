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
//   shallowMount(LibraryView, { global: { plugins: [harness.pinia()] } });
//   vi.mock('./composables/useDocumentsManager', () =>
//     harness.documentsManager(() => ({ open: openDoc })));
import { vi } from 'vitest';
import { ref } from 'vue';
import { testingPinia } from './testingPinia';
import type { TrajectSummary } from '../stores/trajects';

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

// Login state is settled from the start (`loading` false), as after
// /auth/status has answered; `ensureAuthReady` resolves straight away.
export function auth(overrides?: Overrides<[]>) {
  return {
    useAuth: () => ({ authenticated: ref(true), oidcConfigured: ref(false), loading: ref(false), login: vi.fn(), ...apply(overrides, []) }),
    ensureAuthReady: async () => {},
  };
}

// The trajecten the tests use, so the store finds a name for the active one.
const TEST_TRAJECT_REFS = ['traject-abcd1234', 'traject-aaaa1111', 'traject-bbbb2222', 'traject-99998888'];

/**
 * A testing Pinia for LibraryView: every store action is a spy, and the
 * trajecten store holds the test trajecten, loaded. The active traject comes
 * from the route stub, as in the app.
 */
export function pinia() {
  const trajects: TrajectSummary[] = TEST_TRAJECT_REFS.map((ref, i) => ({
    id: `00000000-0000-0000-0000-00000000000${i}`,
    name: `Traject ${i + 1}`,
    description: '',
    scope: '',
    status: 'bezig',
    role: 'owner',
    ref,
  }));
  return testingPinia({ initialState: { trajects: { trajects, loading: false, error: null } } });
}

export function apiFetchModule(module: Record<string, unknown> = {}) {
  return {
    apiFetch: vi.fn().mockResolvedValue({ ok: true, status: 200, json: async () => [] }),
    apiFetchJson: vi.fn().mockResolvedValue([]),
    ApiError: class ApiError extends Error {},
    ...module,
  };
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
