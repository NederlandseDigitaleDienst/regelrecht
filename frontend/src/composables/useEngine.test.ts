import { describe, it, expect, vi } from 'vitest';

// Mock the API layer so `initEngine` never does a real network/WASM load.
// `apiFetchText` rejects with a sentinel: the test asserts that sentinel
// surfaces, which only happens if `apiFetchText` is actually imported and
// called. A regression that dropped the import (it's used only on the WASM-
// init path, which the dependency-loader tests mock past) would instead make
// `initEngine` throw `ReferenceError: apiFetchText is not defined` - caught here.
vi.mock('../lib/apiFetch', () => ({
  apiFetchText: vi.fn().mockRejectedValue(new Error('SENTINEL_GLUE_FETCH')),
  apiFetchJson: vi.fn().mockResolvedValue([]),
}));

import { useEngine } from './useEngine';

describe('useEngine.initEngine', () => {
  it('uses apiFetchText to fetch the WASM glue (regression: import must exist)', async () => {
    const { initEngine, initError } = useEngine();
    await expect(initEngine()).rejects.toThrow('SENTINEL_GLUE_FETCH');
    // It reached the glue fetch and failed there - not a ReferenceError from a
    // missing import.
    expect(initError.value).toBeInstanceOf(Error);
    expect(initError.value?.message).toBe('SENTINEL_GLUE_FETCH');
  });
});
