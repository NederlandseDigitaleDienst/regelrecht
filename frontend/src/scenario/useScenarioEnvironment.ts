/**
 * The laws a scenario run needs in the engine, and the column types of the data
 * sources they declare.
 *
 * The open regelwerk itself is the editor's to load (with its unsaved edits;
 * see EditorView). This loads what it depends on, plus what the scenarios
 * name: their `Given law ... is loaded` steps and the regelwerk each one
 * executes when that is another one. `revision` goes up each time a load has
 * settled, so a caller re-runs its scenarios on it.
 *
 * The open regelwerk's YAML changes on every keystroke while someone edits; the
 * load follows it ~300ms after the last change, so the panel does not flicker.
 */
import { onBeforeUnmount, ref, watch, type Ref } from 'vue';
import * as yaml from 'js-yaml';
import { useDependencies } from '../composables/useDependencies';
import { loadLawVersions, useEngine, type EditorEngine } from '../composables/useEngine';
import { useCorpus } from '../corpus';
import { useLatest } from '../lib/useLatest';
import { buildExternalFieldTypeMap, type FieldMeta } from '../utils/articleMapping';
import type { buildArticleMap } from '../utils/articleMapping';

/** An article as the article/type maps read it (the maps' own parameter type). */
export type MappedArticle = NonNullable<Parameters<typeof buildArticleMap>[0]>[number];

/** A law document as buildExternalFieldTypeMap reads it. */
type MappedLawDoc = { articles?: readonly MappedArticle[] | null };

export interface ScenarioEnvironmentInputs {
  engine: Readonly<Ref<EditorEngine | null>>;
  /** Whether the engine is initialised. */
  ready: Readonly<Ref<boolean>>;
  /** The open regelwerk's YAML, unsaved edits included; null while it loads. */
  lawYaml: Readonly<Ref<string | null>>;
  /** The open regelwerk's articles, for the data-source column types. */
  articles: Readonly<Ref<readonly MappedArticle[]>>;
  /** The regelwerken the scenarios name, beyond the open one. */
  scenarioLaws: Readonly<Ref<readonly string[]>>;
}

export function useScenarioEnvironment(inputs: ScenarioEnvironmentInputs) {
  const { loading, progress, error, loadAllDependencies, loadImplementors } = useDependencies();
  const corpus = useCorpus();
  /** True once the laws of the latest load are in the engine. */
  const ready = ref(false);
  /** Goes up each time a load has settled. */
  const revision = ref(0);
  const externalFieldTypeMap = ref(new Map<string, FieldMeta>());
  /** The scenario laws the latest load could not put in the engine, with why. */
  const failedLaws = ref(new Map<string, string>());
  const { lawLoadError } = useEngine();

  // Every run records what it fetched: the column type map is built from
  // exactly the ruleworks of this run. The corpus reader caches the versions
  // per traject, so a warm engine does not refetch.
  function versionsFetcher(seen: Map<string, string[]>) {
    return async (lawId: string): Promise<string[]> => {
      const { yamls } = await corpus.versions(lawId);
      seen.set(lawId, yamls);
      return yamls;
    };
  }

  // Tolerates unparseable versions: the cached YAML is not schema-checked.
  function rebuildTypeMap(seen: ReadonlyMap<string, string[]>) {
    const docs: MappedLawDoc[] = [{ articles: inputs.articles.value }];
    for (const [lawId, versions] of seen) {
      for (const v of versions) {
        try {
          const doc = yaml.load(v);
          if (doc && typeof doc === 'object') docs.push(doc as MappedLawDoc);
        } catch (e) {
          console.warn(`Skipped an unparseable cached version of '${lawId}' while building the type map:`, e);
        }
      }
    }
    externalFieldTypeMap.value = buildExternalFieldTypeMap(docs);
  }

  const claim = useLatest();

  async function load() {
    const lawYaml = debouncedYaml.value;
    const engine = inputs.engine.value;
    if (!lawYaml || !inputs.ready.value || !engine) return;
    const isCurrent = claim();
    ready.value = false;
    const failed = new Map<string, string>();
    const seen = new Map<string, string[]>();
    const fetchVersions = versionsFetcher(seen);

    const mainLawId = await loadAllDependencies(lawYaml, engine, fetchVersions);
    if (!isCurrent()) return;

    for (const lawId of new Set(inputs.scenarioLaws.value)) {
      if (lawId === mainLawId) continue;
      try {
        // Fetched even when the engine has it, so the type map sees it.
        const yamls = await fetchVersions(lawId);
        if (!engine.hasLaw(lawId)) loadLawVersions(engine, yamls, lawId);
      } catch (e) {
        console.warn(`Failed to load scenario dependency '${lawId}':`, e);
        failed.set(lawId, e instanceof Error ? e.message : String(e));
      }
      if (!isCurrent()) return;
    }

    // The declared laws are in: the scenarios can run. Implementing
    // regulations (IoC) load in the background, because their corpus scan
    // can be slow and is best effort; the types are rebuilt once they settle.
    // Fire-and-forget is safe: Vue ignores ref writes after unmount and the
    // engine outlives this component.
    failedLaws.value = failed;
    ready.value = true;
    revision.value++;
    rebuildTypeMap(seen);
    if (mainLawId) {
      Promise.resolve(loadImplementors(mainLawId, engine, fetchVersions))
        .then(() => {
          if (!isCurrent()) return;
          rebuildTypeMap(seen);
          revision.value++;
        })
        .catch((e) => console.warn('Implementor load / re-type failed (non-fatal):', e));
    }
  }

  // The first YAML applies at once; a change from an existing one waits for
  // the typing to stop.
  const debouncedYaml = ref(inputs.lawYaml.value);
  let yamlTimer: ReturnType<typeof setTimeout> | undefined;
  watch(inputs.lawYaml, (val, prev) => {
    clearTimeout(yamlTimer);
    if (!prev) {
      debouncedYaml.value = val;
      return;
    }
    yamlTimer = setTimeout(() => { debouncedYaml.value = val; }, 300);
  });

  // The inputs settle on separate ticks while things load; a short delay
  // collapses that burst into one load instead of several abandoned ones.
  let loadTimer: ReturnType<typeof setTimeout> | undefined;
  function scheduleLoad() {
    clearTimeout(loadTimer);
    loadTimer = setTimeout(() => void load(), 30);
  }
  watch(
    [debouncedYaml, inputs.ready, inputs.engine, () => inputs.scenarioLaws.value.join('\n')],
    scheduleLoad,
    { immediate: true },
  );

  onBeforeUnmount(() => {
    clearTimeout(yamlTimer);
    clearTimeout(loadTimer);
  });

  /**
   * Why a regelwerk a scenario runs is not in the engine, once that is known:
   * the engine refused it, or this load could not fetch it. Null while it may
   * still arrive.
   */
  function loadErrorFor(lawId: string): string | null {
    return lawLoadError(lawId) ?? failedLaws.value.get(lawId) ?? null;
  }

  return { ready, revision, loading, progress, error, externalFieldTypeMap, loadErrorFor };
}
