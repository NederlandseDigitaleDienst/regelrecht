import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createOpenScenario, CONFLICT_MESSAGE } from './openScenario';
import { res, stubFetch } from '../test-utils/fetchStubs';

type FetchOptions = { method?: string; headers?: Record<string, string>; body?: string };

const FILE = { trajectRef: 'tr-12345678', lawId: 'wet_a', filename: 'basis.feature' };

const FEATURE = `Feature: Basis

  Scenario: Eerste
    Given the calculation date is "2025-01-01"
    Given parameter "inkomen" is 30000
    When I evaluate "heeft_recht" of "wet_a"
    Then output "heeft_recht" is true

  Scenario: Tweede
    When I evaluate "heeft_recht" of "wet_a"
    Then output "heeft_recht" is false
`;

interface Call {
  url: string;
  opts: FetchOptions;
}

function serve({ onPut }: { onPut?: (opts: FetchOptions) => unknown } = {}) {
  const calls: Call[] = [];
  stubFetch(async (url: string, opts?: FetchOptions) => {
    calls.push({ url, opts: opts ?? {} });
    if (opts?.method === 'PUT') return onPut?.(opts) ?? res({ etag: '"v2"', json: { pr: null, etag: '"v2"' } });
    return res({ body: FEATURE, etag: '"v1"' });
  });
  return calls;
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('createOpenScenario', () => {
  it('reads the file through the traject and parses its scenarios', async () => {
    const calls = serve();
    const doc = createOpenScenario(FILE);
    await doc.load();
    expect(calls[0]!.url).toBe('/api/trajects/tr-12345678/corpus/laws/wet_a/scenarios/basis.feature');
    expect(doc.scenarios.value.map((s) => s.name)).toEqual(['Eerste', 'Tweede']);
    expect(doc.dirty.value).toBe(false);
  });

  it('is dirty while a scenario differs from the file, and clean again once it matches', async () => {
    serve();
    const doc = createOpenScenario(FILE);
    await doc.load();
    const original = doc.values.value[0]!;
    doc.setValues(0, { ...original, parameterValues: { inkomen: 1 } });
    expect(doc.dirty.value).toBe(true);
    expect(doc.dirtyIndexes.value).toEqual([0]);
    doc.setValues(0, original);
    expect(doc.dirty.value).toBe(false);
  });

  it('runs the edits before they are saved', async () => {
    serve();
    const doc = createOpenScenario(FILE);
    await doc.load();
    doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: 45000 } });
    expect(doc.inputsFor(0)?.parameters).toEqual({ inkomen: 45000 });
  });

  it('discards edits back to the file', async () => {
    serve();
    const doc = createOpenScenario(FILE);
    await doc.load();
    doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: 1 } });
    doc.discard();
    expect(doc.dirty.value).toBe(false);
    expect(doc.inputsFor(0)?.parameters).toEqual({ inkomen: 30000 });
  });

  it('sends the ETag it read as If-Match and chains the new one', async () => {
    const calls = serve();
    const doc = createOpenScenario(FILE);
    await doc.load();
    doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: 1 } });
    expect(await doc.save()).toBe(true);
    doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: 2 } });
    expect(await doc.save()).toBe(true);

    const puts = calls.filter((c) => c.opts.method === 'PUT');
    expect(puts[0]!.opts.headers?.['If-Match']).toBe('"v1"');
    expect(puts[1]!.opts.headers?.['If-Match']).toBe('"v2"');
    expect(doc.dirty.value).toBe(false);
  });

  it('writes only the edited scenario', async () => {
    const calls = serve();
    const doc = createOpenScenario(FILE);
    await doc.load();
    doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: 1 } });
    await doc.save();
    const body = calls.find((c) => c.opts.method === 'PUT')!.opts.body!;
    expect(body).toContain('Given parameter "inkomen" is 1');
    // The second scenario had no date and gets none from the save.
    const second = body.slice(body.indexOf('Scenario: Tweede'));
    expect(second).not.toContain('calculation date');
  });

  it('surfaces a 412 as a conflict and keeps the edits', async () => {
    serve({ onPut: () => res({ ok: false, status: 412 }) });
    const doc = createOpenScenario(FILE);
    await doc.load();
    doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: 1 } });
    expect(await doc.save()).toBe(false);
    expect(doc.saveError.value?.message).toBe(CONFLICT_MESSAGE);
    expect(doc.dirty.value).toBe(true);
  });

  it('does not read the file again over unsaved edits', async () => {
    const calls = serve();
    const doc = createOpenScenario(FILE);
    await doc.load();
    doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: 1 } });
    await doc.load({ fresh: true });
    expect(calls).toHaveLength(1);
    expect(doc.dirty.value).toBe(true);
  });

  it('finds a scenario by name, else the first', async () => {
    serve();
    const doc = createOpenScenario(FILE);
    await doc.load();
    expect(doc.indexOfName('Tweede')).toBe(1);
    expect(doc.indexOfName('Bestaat niet')).toBe(0);
    expect(doc.indexOfName(null)).toBe(0);
  });

  it('reports a failed read', async () => {
    stubFetch(async () => res({ ok: false, status: 404 }));
    const doc = createOpenScenario(FILE);
    await doc.load();
    expect(doc.error.value).toBeTruthy();
    expect(doc.loaded.value).toBe(false);
  });

  it('keeps what was typed while a save was on its way', async () => {
    let answer: (value: unknown) => void = () => {};
    stubFetch(async (_url: string, opts?: FetchOptions) => {
      if (opts?.method === 'PUT') {
        return new Promise((resolve) => { answer = resolve; });
      }
      return res({ body: FEATURE, etag: '"v1"' });
    });
    const doc = createOpenScenario(FILE);
    await doc.load();
    doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: 1 } });
    const saving = doc.save();
    // Typed after Opslaan, before the server answered.
    doc.setValues(1, { ...doc.values.value[1]!, calculationDate: '2030-01-01' });
    answer(res({ etag: '"v2"', json: { pr: null, etag: '"v2"' } }));
    expect(await saving).toBe(true);

    expect(doc.featureText.value).toContain('Given parameter "inkomen" is 1');
    expect(doc.dirtyIndexes.value).toEqual([1]);
    expect(doc.values.value[1]!.calculationDate).toBe('2030-01-01');
  });

  it('loads the newest version after a conflict, dropping the edits', async () => {
    let version = 'Feature: Basis\n\n  Scenario: Eerste\n    Given parameter "inkomen" is 1\n    When I evaluate "x" of "wet_a"\n';
    stubFetch(async (_url: string, opts?: FetchOptions) => {
      if (opts?.method === 'PUT') return res({ ok: false, status: 412 });
      return res({ body: version, etag: '"v1"' });
    });
    const doc = createOpenScenario(FILE);
    await doc.load();
    doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: 5 } });
    expect(await doc.save()).toBe(false);

    version = version.replace('is 1', 'is 9');
    await doc.reload();
    expect(doc.dirty.value).toBe(false);
    expect(doc.saveError.value).toBeNull();
    expect(doc.inputsFor(0)?.parameters).toEqual({ inkomen: 9 });
  });

  it('finishes a load that a save overtook, without its answer overwriting the edits', async () => {
    let answerLoad: (value: unknown) => void = () => {};
    let reads = 0;
    stubFetch(async (_url: string, opts?: FetchOptions) => {
      if (opts?.method === 'PUT') return res({ etag: '"v2"', json: { pr: null, etag: '"v2"' } });
      reads++;
      if (reads === 1) return res({ body: FEATURE, etag: '"v1"' });
      return new Promise((resolve) => { answerLoad = resolve; });
    });
    const doc = createOpenScenario(FILE);
    await doc.load();

    // Laad nieuwste versie, then an edit and Opslaan before the read answers.
    const reloading = doc.reload();
    doc.setValues(0, { ...doc.values.value[0]!, parameterValues: { inkomen: 7 } });
    expect(await doc.save()).toBe(true);
    answerLoad(res({ body: FEATURE, etag: '"v1"' }));
    await reloading;

    expect(doc.loading.value).toBe(false);
    expect(doc.featureText.value).toContain('Given parameter "inkomen" is 7');
  });
});

describe('duplicate scenario names', () => {
  it('names the ones more than one scenario carries', async () => {
    const twice = `Feature: Dubbel

  Scenario: Basis
    Given parameter "inkomen" is 1
    When I evaluate "x" of "wet_a"

  Scenario: Anders
    Given parameter "inkomen" is 2
    When I evaluate "x" of "wet_a"

  Scenario: Basis
    Given parameter "inkomen" is 3
    When I evaluate "x" of "wet_a"
`;
    stubFetch(async () => res({ body: twice, etag: '"v1"' }));
    const doc = createOpenScenario(FILE);
    await doc.load();
    expect(doc.duplicateNames.value).toEqual(['Basis']);
    // Still read in full: every scenario is there, by position.
    expect(doc.scenarios.value.map((s) => s.name)).toEqual(['Basis', 'Anders', 'Basis']);
    expect(doc.inputsFor(2)?.parameters).toEqual({ inkomen: 3 });
  });

  it('has none in a file whose names differ', async () => {
    stubFetch(async () => res({ body: FEATURE, etag: '"v1"' }));
    const doc = createOpenScenario(FILE);
    await doc.load();
    expect(doc.duplicateNames.value).toEqual([]);
  });
});
