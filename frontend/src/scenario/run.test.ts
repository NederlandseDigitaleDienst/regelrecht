import { describe, it, expect, vi } from 'vitest';
import { runScenario, runStatus, type RunEngine } from './run';
import type { EngineInputs } from './inputs';
import type { FormAssertion } from '../gherkin/formMapper';

function engine(execute: RunEngine['executeWithTrace']): RunEngine & Record<string, ReturnType<typeof vi.fn>> {
  return {
    clearDataSources: vi.fn(),
    registerDataSource: vi.fn(),
    registerDataSourceForLaw: vi.fn(),
    executeWithTrace: vi.fn(execute),
  };
}

const INPUTS: EngineInputs = {
  lawId: 'wet_a',
  outputName: 'heeft_recht',
  calculationDate: '2025-01-01',
  parameters: { inkomen: 1 },
  dataSources: [
    { sourceName: 'globaal', keyField: 'bsn', records: [{ bsn: 1 }] },
    { sourceName: 'per_wet', keyField: 'bsn', lawId: 'wet_b', records: [{ bsn: 2 }] },
  ],
};

const EXPECTS_TRUE: FormAssertion[] = [{ assertionType: 'boolean', outputName: 'heeft_recht', value: true }];

describe('runScenario', () => {
  it('registers the sources, global and per regelwerk, and executes the scenario', () => {
    const e = engine(() => ({ outputs: { heeft_recht: true }, trace_text: 'trace' }));
    const run = runScenario(e, INPUTS, EXPECTS_TRUE);
    expect(e.clearDataSources).toHaveBeenCalled();
    expect(e.registerDataSource).toHaveBeenCalledWith('globaal', 'bsn', [{ bsn: 1 }]);
    expect(e.registerDataSourceForLaw).toHaveBeenCalledWith('wet_b', 'per_wet', 'bsn', [{ bsn: 2 }]);
    expect(e.executeWithTrace).toHaveBeenCalledWith('wet_a', 'heeft_recht', { inkomen: 1 }, '2025-01-01');
    expect(run.traceText).toBe('trace');
    expect(run.error).toBeNull();
    expect(runStatus(run)).toBe('passed');
  });

  it('reports an engine error with its partial trace instead of throwing', () => {
    const e = engine(() => {
      throw { error: 'Law not found', trace_text: 'partial' };
    });
    const run = runScenario(e, INPUTS, EXPECTS_TRUE);
    expect(run.error).toBe('Law not found');
    expect(run.traceText).toBe('partial');
    expect(runStatus(run)).toBe('failed');
  });

  it('reads any thrown value as an error message', () => {
    expect(runScenario(engine(() => { throw 'kapot'; }), INPUTS, []).error).toBe('kapot');
    expect(runScenario(engine(() => { throw new Error('stuk'); }), INPUTS, []).error).toBe('stuk');
    expect(runScenario(engine(() => { throw null; }), INPUTS, []).error).toBe('null');
  });

  it('does not run a scenario that names no output', () => {
    const e = engine(() => ({}));
    const run = runScenario(e, { ...INPUTS, outputName: null }, []);
    expect(e.executeWithTrace).not.toHaveBeenCalled();
    expect(run.error).toBeTruthy();
  });
});

describe('runStatus', () => {
  it('fails a run whose output differs from the expectation', () => {
    const run = runScenario(engine(() => ({ outputs: { heeft_recht: false } })), INPUTS, EXPECTS_TRUE);
    expect(runStatus(run)).toBe('failed');
  });

  it('says so when the scenario expects nothing', () => {
    const run = runScenario(engine(() => ({ outputs: { heeft_recht: false } })), INPUTS, []);
    expect(runStatus(run)).toBe('no-expectations');
  });

  it('is null before a run', () => {
    expect(runStatus(null)).toBeNull();
  });
});
