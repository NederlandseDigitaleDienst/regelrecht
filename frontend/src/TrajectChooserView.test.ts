// Picking a traject carries the law the user was heading for (`?law=`,
// `?article=`) into the traject-scoped route. A repeated key arrives as an
// array; passed into a route param as is, vue-router throws on resolving it.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount } from '@vue/test-utils';
import type { LocationQuery, RouteLocationRaw } from 'vue-router';
import appRouter from './router';
import TrajectChooserView from './TrajectChooserView.vue';
import { testingPinia } from './test-utils/testingPinia';

const routeState: { query: LocationQuery } = { query: {} };
const push = vi.fn<(to: RouteLocationRaw) => void>();

vi.mock('vue-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('vue-router')>()),
  useRoute: () => routeState,
  useRouter: () => ({ push }),
}));

const REF = 'mijn-traject-1a2b3c4d';

interface ChooserInternals {
  selectTraject(t: { ref: string }): void;
}

function selectWithQuery(query: LocationQuery): RouteLocationRaw {
  routeState.query = query;
  const wrapper = shallowMount(TrajectChooserView, {
    global: { plugins: [testingPinia({ initialState: { trajects: { trajects: [], loading: false, error: null } } })] },
  });
  (wrapper.vm as unknown as ChooserInternals).selectTraject({ ref: REF });
  expect(push).toHaveBeenCalledTimes(1);
  return push.mock.calls[0]![0];
}

describe('TrajectChooserView - repeated query values', () => {
  beforeEach(() => push.mockReset());

  it('opens the first law of a repeated ?law= in the editor', () => {
    const target = selectWithQuery({ law: ['foo', 'bar'], article: ['3', '4'] });
    expect(() => appRouter.resolve(target)).not.toThrow();
    expect(appRouter.resolve(target).path).toBe(`/trajecten/${REF}/editor/foo/3`);
  });

  it('opens the first law of a repeated ?law= in the library', () => {
    const target = selectWithQuery({ sectie: 'library', law: ['foo', 'bar'] });
    expect(() => appRouter.resolve(target)).not.toThrow();
    expect(appRouter.resolve(target).path).toBe(`/trajecten/${REF}/corpus/foo`);
  });

  it('treats a valueless ?law as no law', () => {
    const target = selectWithQuery({ law: null });
    expect(appRouter.resolve(target).path).toBe(`/trajecten/${REF}/editor`);
  });
});
