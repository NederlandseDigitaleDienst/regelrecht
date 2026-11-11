// A Pinia for component tests, as the Pinia testing guide recommends: every
// action is replaced by a `vi.fn` spy, and `initialState` sets the state of a
// store by its id before the component reads it. Pass it in
// `global.plugins`; it also becomes the active Pinia, so `use…Store()` in the
// test reaches the same store the component uses.
//
// Stubbing has one limit in setup stores: an action that calls another action
// of the same store calls the real function. The guide's advice is to test the
// real code there rather than work around it.
import { vi, type Mock } from 'vitest';
import type { UnwrapRef } from 'vue';
import type { Store, StoreDefinition } from 'pinia';
import { createTestingPinia, type TestingOptions } from '@pinia/testing';
import { DEFAULTS } from '../stores/featureFlags';

export function testingPinia(options: Omit<TestingOptions, 'createSpy'> = {}) {
  return createTestingPinia({ createSpy: vi.fn, ...options });
}

/** Every known feature flag switched on, as `initialState` for the featureFlags store. */
export function allFlagsOn(): { flags: Record<string, boolean> } {
  return { flags: Object.fromEntries(Object.keys(DEFAULTS).map((key) => [key, true])) };
}

/**
 * A store with its actions typed as Vitest mocks, for a test that sets what a
 * stubbed action returns (`store.refresh.mockResolvedValue(...)`). The helper
 * from the Pinia testing guide; the actions are already spies at runtime.
 */
export function mockedStore<TStoreDef extends () => unknown>(
  useStore: TStoreDef,
): TStoreDef extends StoreDefinition<infer Id, infer State, infer Getters, infer Actions>
  ? Store<
      Id,
      State,
      Record<string, never>,
      {
        [K in keyof Actions]: Actions[K] extends (...args: never[]) => unknown ? Mock<Actions[K]> : Actions[K];
      }
    > & { [K in keyof Getters]: UnwrapRef<Getters[K]> }
  : ReturnType<TStoreDef> {
  // The runtime value is the store itself; only its type is narrowed.
  return useStore() as never;
}
