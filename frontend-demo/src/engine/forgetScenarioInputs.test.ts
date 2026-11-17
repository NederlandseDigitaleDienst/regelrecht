import { describe, it, expect, vi } from 'vitest';
import { forgetScenarioInputs } from './useDemoEngine';

describe('forgetScenarioInputs', () => {
  it('clears the territories with the data sources, so neither reaches the next scenario', () => {
    const engine = { clearDataSources: vi.fn(), clearTerritories: vi.fn() };
    forgetScenarioInputs(engine);
    expect(engine.clearDataSources).toHaveBeenCalledTimes(1);
    expect(engine.clearTerritories).toHaveBeenCalledTimes(1);
  });
});
