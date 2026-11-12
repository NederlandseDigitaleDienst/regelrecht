// After creating a traject the view opens the law the user was heading for
// (`?law=`, `?article=`). A repeated key arrives as an array; passed into a
// route param as is, vue-router throws on resolving it.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { flushPromises, shallowMount } from '@vue/test-utils';
import { defineComponent } from 'vue';
import type { LocationQuery, RouteLocationRaw } from 'vue-router';
import appRouter from './router';
import TrajectCreateView from './TrajectCreateView.vue';
import { useTrajectsStore } from './stores/trajects';
import { mockedStore, testingPinia } from './test-utils/testingPinia';

const routeState: { query: LocationQuery } = { query: {} };
const push = vi.fn<(to: RouteLocationRaw) => Promise<void>>();

vi.mock('vue-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('vue-router')>()),
  useRoute: () => routeState,
  useRouter: () => ({ push }),
}));

const REF = 'mijn-traject-1a2b3c4d';

// The real form validates its fields; this one always hands a payload.
const FormStub = defineComponent({
  name: 'TrajectCreateForm',
  setup(_, { expose }) {
    expose({ buildPayload: () => ({ payload: { name: 'Mijn traject' }, error: null }) });
    return () => null;
  },
});

interface CreateInternals {
  submitCreate(): Promise<void>;
}

async function createWithQuery(query: LocationQuery): Promise<RouteLocationRaw> {
  routeState.query = query;
  const wrapper = shallowMount(TrajectCreateView, {
    global: { stubs: { TrajectCreateForm: FormStub }, plugins: [testingPinia()] },
  });
  mockedStore(useTrajectsStore).createTraject.mockResolvedValue({
    id: '00000000-0000-0000-0000-0000001a2b3c',
    name: 'Mijn traject',
    description: '',
    scope: '',
    status: 'bezig',
    role: 'owner',
    ref: REF,
  });
  await (wrapper.vm as unknown as CreateInternals).submitCreate();
  await flushPromises();
  expect(push).toHaveBeenCalledTimes(1);
  return push.mock.calls[0]![0];
}

describe('TrajectCreateView - repeated query values', () => {
  beforeEach(() => {
    push.mockReset();
    push.mockResolvedValue(undefined);
  });

  it('opens the first law of a repeated ?law= in the new traject', async () => {
    const target = await createWithQuery({ law: ['foo', 'bar'], article: ['3', '4'] });
    expect(() => appRouter.resolve(target)).not.toThrow();
    expect(appRouter.resolve(target).path).toBe(`/trajecten/${REF}/editor/foo/3`);
  });
});
