import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useDependencies, extractRegulationRefs } from './useDependencies';
import { res, stubFetch } from '../test-utils/fetchStubs';

// Fake WASM engine: tracks which law ids are loaded, plus every body handed to
// `loadLaw` so multi-version loading can be asserted (the real engine keys
// versions by ($id, valid_from), so several bodies of one id coexist).
function fakeEngine() {
  const loaded = new Set<string>();
  const bodies: string[] = [];
  return {
    loaded,
    bodies,
    hasLaw: (id: string) => loaded.has(id),
    loadLaw: (yamlText: string) => {
      // Model the real WASM engine rejecting an unparseable version (e.g. an
      // old-schema or text-only harvested version): a body marked UNLOADABLE
      // throws, and must not register the law.
      if (yamlText.includes('UNLOADABLE')) throw new Error('unparseable version');
      bodies.push(yamlText);
      const m = yamlText.match(/\$id:\s*(\S+)/);
      if (m) loaded.add(m[1] as string);
    },
  };
}

const MAIN_LAW = `\
$id: wet_op_de_zorgtoeslag
articles:
  - number: '2'
    machine_readable:
      execution:
        input:
          - name: dekking
            source:
              regulation: zorgverzekeringswet
              output: dekking
`;

// Version lists per law id (the `fetchLawVersions` contract). No further refs,
// so the walk terminates.
const DEP_VERSIONS: Record<string, string[]> = {
  zorgverzekeringswet: ['$id: zorgverzekeringswet\narticles: []\n'],
  regeling_standaardpremie: ['$id: regeling_standaardpremie\narticles: []\n'],
};

// The walk only asks for ids in DEP_VERSIONS; an unknown id yields undefined,
// as the untyped lookup did before.
function depVersions(id: string): string[] {
  return DEP_VERSIONS[id] as string[];
}

// Test laws are plain literals with extra keys (`number`, `name`, ...).
type LawArg = Parameters<typeof extractRegulationRefs>[0];
const asLaw = (law: unknown) => law as LawArg;

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('extractRegulationRefs', () => {
  it('collects source.regulation refs and skips self-references', () => {
    expect(extractRegulationRefs(asLaw(yamlMain()))).toEqual(['zorgverzekeringswet']);
  });
  it('collects the regulation of a fixed source.article target, not of a $-target', () => {
    const law = {
      $id: 'wet_heroverweging',
      articles: [
        {
          number: '1',
          machine_readable: {
            execution: {
              input: [
                { name: 'a', source: { article: 'wet_verlof#1', output: 'verleend' } },
                { name: 'b', source: { article: '$besluit.basis' } },
                { name: 'c', source: { article: 'wet_heroverweging#2' } },
              ],
            },
          },
        },
      ],
    };
    expect(extractRegulationRefs(asLaw(law))).toEqual(['wet_verlof']);
  });
  it('returns [] for a law with no machine_readable inputs', () => {
    expect(extractRegulationRefs(asLaw({ $id: 'x', articles: [{ number: '1' }] }))).toEqual([]);
  });
});

describe('useDependencies.loadAllDependencies', () => {
  it('loads source.regulation deps, returns the $id, and does NOT scan the corpus', async () => {
    // The implementor scan is off the critical path, so loading the law's own
    // deps must make no network call at all (only fetchLawVersions is used).
    const fetchSpy = vi.fn();
    stubFetch(fetchSpy);
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (id: string) => depVersions(id));

    const { loadAllDependencies, loading } = useDependencies();
    const mainLawId = await loadAllDependencies(MAIN_LAW, engine, fetchLawVersions);

    expect(mainLawId).toBe('wet_op_de_zorgtoeslag');
    expect(engine.loaded.has('zorgverzekeringswet')).toBe(true);
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(loading.value).toBe(false);
  });

  it('loads EVERY version of a dependency (in-force + future), not just one', async () => {
    // Regression for the temporal-federation bug: a referenced law that has a
    // future-dated version must have *all* its versions loaded so the engine
    // can pick the one in force on the scenario date - loading only the future
    // version would fail "not yet in force" for a past-dated scenario.
    const inForce = '$id: zorgverzekeringswet\nvalid_from: \'2025-01-01\'\narticles: []\n';
    const future = '$id: zorgverzekeringswet\nvalid_from: \'2099-01-01\'\narticles: []\n';
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (id: string) =>
      id === 'zorgverzekeringswet' ? [future, inForce] : depVersions(id),
    );

    const { loadAllDependencies } = useDependencies();
    await loadAllDependencies(MAIN_LAW, engine, fetchLawVersions);

    const zvwBodies = engine.bodies.filter((b) => b.includes('$id: zorgverzekeringswet'));
    expect(zvwBodies).toHaveLength(2);
    expect(zvwBodies).toContain(inForce);
    expect(zvwBodies).toContain(future);
  });

  it('isolates an unloadable version: a bad version does not drop the good ones', async () => {
    // Regression for the federated-corpus case where a law has an executable
    // version AND an old-schema / text-only harvested version the engine can't
    // parse. The harvested version must not abort the whole law - the engine
    // must still hold the executable one so select_in can resolve it.
    const bad = '$id: zorgverzekeringswet\nvalid_from: \'2026-02-21\'\nUNLOADABLE: true\n';
    const good = '$id: zorgverzekeringswet\nvalid_from: \'2025-01-01\'\narticles: []\n';
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (id: string) =>
      id === 'zorgverzekeringswet' ? [bad, good] : depVersions(id),
    );

    const { loadAllDependencies } = useDependencies();
    await loadAllDependencies(MAIN_LAW, engine, fetchLawVersions);

    expect(engine.loaded.has('zorgverzekeringswet')).toBe(true);
    expect(engine.bodies).toContain(good);
    expect(engine.bodies).not.toContain(bad);
  });

  it('treats an empty version list as a missing dependency (requests harvest)', async () => {
    // /versions returning [] means no version is available - the loader must
    // not silently succeed; it routes the id to the harvest request like a
    // fetch failure. The harvest call is the only network request.
    const fetchSpy = vi.fn(async (url: string) => {
      if (String(url).includes('/harvest')) return res({ json: { results: [] } });
      return res({ ok: false, status: 404, json: [] });
    });
    stubFetch(fetchSpy);
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (): Promise<string[]> => []);

    const { loadAllDependencies } = useDependencies();
    await loadAllDependencies(MAIN_LAW, engine, fetchLawVersions);

    expect(engine.loaded.has('zorgverzekeringswet')).toBe(false);
    // The missing dep is routed to harvest, not silently dropped.
    const harvestCalls = fetchSpy.mock.calls.filter((c) => String(c[0]).includes('/harvest'));
    expect(harvestCalls.length).toBeGreaterThan(0);
  });

  it('refetches versions for a dep already in the (warm) engine, without reloading it', async () => {
    // Regression for the data-source typing bug: the shared WASM engine
    // persists across scenario-panel mounts (and is pre-warmed by the machine
    // view), so a dependency is often already loaded by the time the panel
    // resolves its graph. The data-source column type map is built from the
    // fetched version YAMLs (versionsCache), NOT from the engine - so an
    // already-loaded dep must STILL have its versions fetched. Gating the fetch
    // on `engine.hasLaw` left the cache empty on a warm engine and collapsed
    // every typed cell (boolean dropdown, euro field, date picker) to a plain
    // string field. The engine load itself is correctly skipped (already there).
    const engine = fakeEngine();
    engine.loaded.add('zorgverzekeringswet'); // pre-warm: dep already loaded
    const fetchLawVersions = vi.fn(async (id: string) => depVersions(id));

    const { loadAllDependencies } = useDependencies();
    await loadAllDependencies(MAIN_LAW, engine, fetchLawVersions);

    expect(fetchLawVersions).toHaveBeenCalledWith('zorgverzekeringswet');
    // Not re-loaded into the engine: no body was handed to loadLaw for it.
    expect(engine.bodies.some((b) => b.includes('$id: zorgverzekeringswet'))).toBe(false);
  });

  it('does not chase transitive deps of a fetched-but-unloadable (missing) law', async () => {
    // The transitive-ref scan must run only for a "usable" law (already loaded,
    // or newly loaded). A law whose versions are all fetched but unloadable is
    // missing/broken - chasing its refs would queue deps of a law that can't
    // run. (Regression for moving the scan out of the try/catch.)
    const unloadableZvw = [
      '$id: zorgverzekeringswet',
      'UNLOADABLE: true',
      'articles:',
      "  - number: '1'",
      '    machine_readable:',
      '      execution:',
      '        input:',
      '          - name: x',
      '            source:',
      '              regulation: regeling_standaardpremie',
      '              output: y',
    ].join('\n') + '\n';
    const fetchSpy = vi.fn(async (url: string) => {
      if (String(url).includes('/harvest')) return res({ json: { results: [] } });
      return res({ ok: false, status: 404, json: [] });
    });
    stubFetch(fetchSpy);
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (id: string) =>
      id === 'zorgverzekeringswet' ? [unloadableZvw] : depVersions(id),
    );

    const { loadAllDependencies } = useDependencies();
    await loadAllDependencies(MAIN_LAW, engine, fetchLawVersions);

    // The unloadable law is missing (never registered in the engine)...
    expect(engine.loaded.has('zorgverzekeringswet')).toBe(false);
    expect(fetchLawVersions).toHaveBeenCalledWith('zorgverzekeringswet');
    // ...and its transitive ref was NOT chased.
    expect(fetchLawVersions).not.toHaveBeenCalledWith('regeling_standaardpremie');
  });
});

describe('useDependencies.loadImplementors', () => {
  it('resolves implementors with a single request and loads them', async () => {
    const fetchSpy = vi.fn().mockImplementation(async (url: string) => {
      if (url.endsWith('/implementors')) return res({ json: ['regeling_standaardpremie'] });
      return res({ ok: false, status: 404, json: [] });
    });
    stubFetch(fetchSpy);
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (id: string) => depVersions(id));

    const { loadImplementors } = useDependencies();
    await loadImplementors('wet_op_de_zorgtoeslag', engine, fetchLawVersions, 'mig-1a2b3c4d');

    expect(fetchSpy).toHaveBeenCalledTimes(1);
    expect(fetchSpy.mock.calls[0]?.[0]).toBe(
      '/api/trajects/mig-1a2b3c4d/corpus/laws/wet_op_de_zorgtoeslag/implementors',
    );
    expect(fetchSpy.mock.calls.some((c) => String(c[0]).includes('limit=1000'))).toBe(false);
    expect(engine.loaded.has('regeling_standaardpremie')).toBe(true);
  });

  it('runs at most once per (trajectRef, lawId)', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(res({ json: ['regeling_standaardpremie'] }));
    stubFetch(fetchSpy);
    const engine = fakeEngine();
    const fetchLawVersions = vi.fn(async (id: string) => depVersions(id));

    const { loadImplementors } = useDependencies();
    await loadImplementors('wet_op_de_zorgtoeslag', engine, fetchLawVersions, 'mig-1a2b3c4d');
    await loadImplementors('wet_op_de_zorgtoeslag', engine, fetchLawVersions, 'mig-1a2b3c4d');

    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it('is best-effort: a failed scan resolves without throwing and can be retried', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(res({ ok: false, status: 500 }));
    stubFetch(fetchSpy);
    const engine = fakeEngine();
    const { loadImplementors } = useDependencies();
    await expect(
      loadImplementors('wet_op_de_zorgtoeslag', engine, vi.fn(), null),
    ).resolves.toBeUndefined();
    // A transient failure must not permanently suppress the scan: a second
    // call retries (the guard key was reset on failure).
    await loadImplementors('wet_op_de_zorgtoeslag', engine, vi.fn(), null);
    expect(fetchSpy).toHaveBeenCalledTimes(2);
  });
});

// Parsed equivalent of MAIN_LAW (plus a self-reference) for the pure test.
function yamlMain() {
  return {
    $id: 'wet_op_de_zorgtoeslag',
    articles: [
      {
        number: '2',
        machine_readable: {
          execution: {
            input: [
              { name: 'dekking', source: { regulation: 'zorgverzekeringswet', output: 'dekking' } },
              { name: 'self', source: { regulation: 'wet_op_de_zorgtoeslag', output: 'x' } },
            ],
          },
        },
      },
    ],
  };
}
