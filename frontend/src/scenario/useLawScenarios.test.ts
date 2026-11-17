import { describe, it, expect, beforeEach, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent, ref, type Ref } from 'vue';
import { useLawScenarios, type LawScenarios } from './useLawScenarios';
import { installTestCorpus } from '../test-utils/corpusSetup';
import { res, stubFetch } from '../test-utils/fetchStubs';
import type { EditorEngine } from '../composables/useEngine';

const TRAJECT = 'tr-12345678';
const LAW_YAML = '$id: wet_a\narticles: []\n';

const engine = {
  hasLaw: () => true,
  loadLaw: () => '',
  unloadLaw: () => true,
  clearDataSources: () => {},
  registerDataSource: () => {},
  registerDataSourceForLaw: () => {},
  executeWithTrace: () => ({ outputs: {} }),
} as unknown as EditorEngine;

let requested: string[] = [];
// Every read of the corpus: dependency loads and the implementor scan go here.
let corpusReads: string[] = [];

function mountWith(lawId: Ref<string | null>): LawScenarios {
  let scenarios: LawScenarios | null = null;
  mount(defineComponent({
    setup() {
      scenarios = useLawScenarios({
        trajectRef: ref(TRAJECT),
        lawId,
        engine: ref(engine),
        engineReady: ref(true),
        lawYaml: ref(LAW_YAML),
        articles: ref([]),
        lawLoading: ref(false),
      });
      return () => null;
    },
  }));
  // Set in setup, which mount runs synchronously.
  return scenarios!;
}

async function settle() {
  await flushPromises();
  await vi.advanceTimersByTimeAsync(400);
  await flushPromises();
}

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  requested = [];
  corpusReads = [];
  const { transport } = installTestCorpus({ [TRAJECT]: { laws: { wet_a: LAW_YAML } } }, TRAJECT);
  for (const name of ['law', 'versions', 'outputs', 'implementors', 'list', 'changedLaws'] as const) {
    const original = transport[name].bind(transport) as (...a: unknown[]) => unknown;
    vi.spyOn(transport, name).mockImplementation(((...a: unknown[]) => {
      corpusReads.push(name);
      return original(...a);
    }) as never);
  }
  stubFetch(async (url: string) => {
    requested.push(url);
    if (url.endsWith('/scenarios')) return res({ json: [] });
    return res({ json: [] });
  });
});

describe('useLawScenarios', () => {
  it('does nothing without a regelwerk', async () => {
    const s = mountWith(ref(null));
    await settle();
    expect(requested).toEqual([]);
    expect(corpusReads).toEqual([]);
    expect(s.loading.value).toBe(false);
    expect(s.files.value).toEqual([]);
  });

  it('lists the files of a regelwerk without scenarios, and loads nothing else', async () => {
    const s = mountWith(ref('wet_a'));
    await settle();
    expect(requested).toHaveLength(1);
    expect(requested[0]).toMatch(/\/scenarios$/);
    // No dependency walk and no implementor scan for a regelwerk without scenarios.
    expect(corpusReads).toEqual([]);
    expect(s.loading.value).toBe(false);
    expect(s.environmentError.value).toBeNull();
  });
});
