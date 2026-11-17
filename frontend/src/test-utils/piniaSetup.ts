// Makes a fresh Pinia active before every test, so a store starts empty in
// each test and any `use…Store()` call - in a store test or in a mounted
// component - finds it without further setup. This is the store-test setup
// the Pinia testing guide gives. A component test that wants stubbed actions
// or a given starting state mounts with `createTestingPinia` instead (see
// testingPinia.ts).
import { beforeEach } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';

beforeEach(() => {
  setActivePinia(createPinia());
});
