/**
 * useDependencies - recursive dependency graph walker for law YAML files.
 *
 * Parses a law's YAML structure to find all `source.regulation` references,
 * fetches each dependency via the API, loads it into the engine, and recurses.
 * Implementing regulations (the IoC reverse link) are loaded separately via
 * `loadImplementors`, which calls the backend `implementors` endpoint - kept
 * OFF the critical path because that corpus scan can be slow on a large
 * federated corpus and scenarios already declare the regulations they need.
 */
import { ref } from 'vue';
import * as yaml from 'js-yaml';
import { useBwbHarvest } from './useBwbHarvest';
import { loadLawVersions } from './useEngine';
import { useCorpus } from '../corpus';

/** The fields of a parsed law that the dependency walk reads. */
interface LawDependencyFields {
  $id?: string | null;
  articles?:
    | {
        machine_readable?: {
          execution?: {
            input?: { source?: { regulation?: string | null; article?: unknown } | null }[] | null;
          } | null;
        } | null;
      }[]
    | null;
}

/** The engine methods the walk calls; loadLaw's return value is unused. */
interface DependencyEngine {
  hasLaw(lawId: string): boolean;
  loadLaw(yaml: string): unknown;
}

type FetchLawVersions = (lawId: string) => Promise<string[]>;

/**
 * Extract all unique `source.regulation` references from a parsed law object,
 * and the regulation of a fixed `source.article` target (`<regulation>#<article>`,
 * schema v0.8.4). A `$`-target names its article only at run time, so it has
 * no regulation to preload. Skips self-references (where regulation === the
 * law's own $id).
 *
 * @param {object} law - Parsed law YAML object
 * @returns {string[]} Array of unique law IDs referenced
 */
export function extractRegulationRefs(law: LawDependencyFields): string[] {
  const refs = new Set<string>();
  const selfId = law.$id;

  for (const article of law.articles || []) {
    const inputs = article.machine_readable?.execution?.input || [];
    for (const input of inputs) {
      const target = input.source?.article;
      const reg =
        input.source?.regulation ??
        (typeof target === 'string' && !target.startsWith('$') ? target.split('#')[0] : null);
      if (reg && reg !== selfId) {
        refs.add(reg);
      }
    }
  }

  return [...refs];
}

/**
 * Composable for loading all dependencies of a law recursively.
 */
export function useDependencies() {
  const loading = ref(false);
  const loadedDeps = ref<string[]>([]);
  const progress = ref('');
  const error = ref<string | null>(null);
  const { requestHarvestBatch } = useBwbHarvest();

  // Guards `loadImplementors` so a re-render / re-trigger of the scenario
  // panel doesn't restart the corpus-wide implementor scan for a law it
  // already scanned. Keyed on `{trajectRef}::{lawId}`.
  // The implementors scan for one (traject, law), shared by every run for it.
  // Null when the scan failed, so the next trigger retries it.
  let implementorsKey: string | null = null;
  let implementorsScan: Promise<string[] | null> | null = null;

  /**
   * Load a law's direct + transitive `source.regulation` dependencies into
   * the engine. Returns the law's `$id` (or null) so the caller can kick off
   * the off-critical-path `loadImplementors` scan.
   *
   * `fetchLawVersions` already resolves through the active traject, so no
   * traject ref is needed here. It returns **all** versions of a law (the
   * engine keys versions by `($id, valid_from)`, so several bodies of one law
   * coexist), and the engine picks the version in force on the scenario's
   * calculation date - a referenced law that has a future-dated version would
   * otherwise load only that future version and fail "not yet in force".
   *
   * @param {string} lawYamlText - Raw YAML text of the main law
   * @param {object} engine - WasmEngine instance
   * @param {(lawId: string) => Promise<string[]>} fetchLawVersions - Fetch all version YAMLs by ID
   * @returns {Promise<string|null>} The main law's `$id`.
   */
  async function loadAllDependencies(
    lawYamlText: string,
    engine: DependencyEngine,
    fetchLawVersions: FetchLawVersions,
  ): Promise<string | null> {
    loading.value = true;
    error.value = null;
    loadedDeps.value = [];
    progress.value = 'Afhankelijkheden analyseren...';

    let mainLawId: string | null = null;
    try {
      // A body that is not a mapping throws on `.$id` and lands in the catch.
      const mainLaw = yaml.load(lawYamlText) as LawDependencyFields;
      mainLawId = mainLaw.$id || null;
      const visited = new Set<string>();
      const toLoad: string[] = [];

      // Phase 1: Collect all transitive regulation references
      collectDeps(mainLaw, visited, toLoad);

      // Phase 2: Load all collected dependencies
      let total = toLoad.length;
      let loaded = 0;
      const missingDeps: string[] = [];

      for (const lawId of toLoad) {
        // Fetch the version YAMLs unconditionally - even for a law already in
        // the engine. They feed two consumers that are independent of engine
        // state: the transitive-ref scan below, and ScenarioBuilder's
        // data-source column type map, built from the versions each run hands
        // over. The WASM engine is a shared singleton that persists across
        // scenario-panel mounts and is pre-warmed by the machine view, so
        // gating this fetch on `engine.hasLaw` would leave the type map empty
        // on a warm engine and collapse every typed data-source cell back to a
        // plain string field. The corpus reader caches the versions.
        const alreadyLoaded = engine.hasLaw(lawId);
        let yamls: string[] = [];
        // Whether this law is sound enough to scan for transitive deps: already
        // in the engine, or newly loaded without error. A law that was fetched
        // but failed to load into the engine is broken/missing - we must NOT
        // chase its transitive refs (would queue deps of a law that can't run).
        let usable = alreadyLoaded;
        try {
          yamls = await fetchLawVersions(lawId);
          // Load every version (isolating each) so the engine's date-aware
          // selection can pick the one in force on the scenario's calculation
          // date. Skip the engine load when the law is already present - only
          // its YAML (cached above) is still needed. For a not-yet-loaded law,
          // no loadable version - empty list or every version unloadable - is a
          // missing dependency (harvest requested below).
          if (!alreadyLoaded && !loadLawVersions(engine, yamls, lawId)) {
            throw new Error(`no loadable version for '${lawId}'`);
          }
          usable = true;
          loaded++;
          loadedDeps.value = [...loadedDeps.value, lawId];
          progress.value = `${loaded}/${total} regelwerken geladen`;
        } catch (e) {
          loaded++;
          if (alreadyLoaded) {
            // Already executable in the engine; only the type-map YAML couldn't
            // be (re)fetched. Don't route it to harvest as if it were missing.
            console.warn(`Could not refetch versions of already-loaded '${lawId}'; type map may be incomplete:`, e);
            loadedDeps.value = [...loadedDeps.value, lawId];
            progress.value = `${loaded}/${total} regelwerken geladen`;
          } else {
            console.warn(`Failed to load dependency '${lawId}':`, e);
            missingDeps.push(lawId);
            progress.value = `${loaded}/${total} regelwerken geladen (${lawId} mislukt)`;
          }
        }

        // Recurse for transitive deps of a usable law only. Collect from every
        // version - a `source.regulation` reference can appear in one version
        // and not another, and a scenario may be evaluated at any calculation
        // date (including a future one), so a reference introduced only by a
        // future version must still be loadable. Runs for already-loaded laws
        // too, so a dep reachable only through one is still discovered and
        // cached. This can over-fetch a transitive law that no in-force version
        // needs; that's an accepted, bounded cost - `select_in` never selects a
        // not-yet-in-force version.
        if (usable) {
          const newDeps: string[] = [];
          for (const versionYaml of yamls) {
            // Isolate the ref scan per version: a version that loaded into the
            // engine but is malformed for `js-yaml` must not throw here.
            try {
              collectDeps(yaml.load(versionYaml) as LawDependencyFields, visited, newDeps);
            } catch (e) {
              console.warn(`Skipped transitive-ref scan of an unparseable version of '${lawId}':`, e);
            }
          }
          if (newDeps.length > 0) {
            toLoad.push(...newDeps);
            total = toLoad.length;
          }
        }
      }

      // Phase 4: Request harvest for missing dependencies
      const defaultProgress = total > 0
        ? `${loadedDeps.value.length}/${total} regelwerken geladen`
        : 'Geen afhankelijkheden';

      if (missingDeps.length > 0) {
        const harvestResult = await requestHarvestBatch(missingDeps);
        const requested = harvestResult?.results?.filter(
          (r) => r.status === 'queued' || r.status === 'already_queued',
        ) ?? [];
        progress.value = requested.length > 0
          ? `${defaultProgress} \u2014 ${requested.length === 1 ? '1 ontbrekend regelwerk' : `${requested.length} ontbrekende regelwerken`} aangevraagd`
          : defaultProgress;
      } else {
        progress.value = defaultProgress;
      }
    } catch (e) {
      error.value = (e as Error).message || String(e);
    } finally {
      loading.value = false;
    }
    return mainLawId;
  }

  /**
   * Load implementing regulations (the IoC reverse link) into the engine.
   *
   * Deliberately OFF the critical path: the backend implementors scan can be
   * slow on a large federated corpus, and scenarios declare the regulations
   * they need explicitly (their `Given law "x" is loaded` background), so the
   * scenario panel must not block on this. The caller fires it without
   * awaiting, after the panel is already usable. The scan runs at most once
   * per `(active traject, lawId)`, so re-renders don't restart it; every run
   * still hands the implementors' versions to `fetchLawVersions`, because the
   * caller builds its type map per run.
   *
   * @param {string|null} lawId - The `$id` of the law to find implementors of.
   * @param {object} engine - WasmEngine instance.
   * @param {(lawId: string) => Promise<string[]>} fetchLawVersions - Fetch all version YAMLs.
   */
  async function loadImplementors(
    lawId: string | null,
    engine: DependencyEngine,
    fetchLawVersions: FetchLawVersions,
  ): Promise<void> {
    if (!lawId) return;
    const corpus = useCorpus();
    const key = `${corpus.trajectRef ?? ''}::${lawId}`;
    if (implementorsKey !== key || !implementorsScan) {
      // Claim the key up front so a concurrent re-trigger joins this scan;
      // reset it on failure so a transient error (network blip, throttled
      // backend) can be retried on the next trigger rather than suppressed
      // for the component's lifetime.
      implementorsKey = key;
      implementorsScan = corpus.implementors(lawId).catch(() => {
        // Best-effort: explicitly-declared deps still cover the common case.
        if (implementorsKey === key) implementorsScan = null;
        return null;
      });
    }
    const implementors = await implementorsScan;
    if (!implementors) return;
    for (const implId of implementors) {
      try {
        // Fetched even when the engine already has it, like the explicit
        // dependencies above: the caller builds its data-source type map from
        // the versions it is handed. The reader caches them.
        const yamls = await fetchLawVersions(implId);
        if (!engine.hasLaw(implId) && loadLawVersions(engine, yamls, implId)) {
          loadedDeps.value = [...loadedDeps.value, implId];
        }
      } catch (e) {
        console.warn(`Failed to load implementing regulation '${implId}':`, e);
      }
    }
  }

  return { loading, loadedDeps, progress, error, loadAllDependencies, loadImplementors };
}

/**
 * Recursively collect dependency law IDs from a parsed law.
 * Mutates `visited` and `toLoad` in place.
 */
function collectDeps(law: LawDependencyFields, visited: Set<string>, toLoad: string[]): void {
  const selfId = law.$id;
  if (selfId) visited.add(selfId);

  const refs = extractRegulationRefs(law);
  for (const depId of refs) {
    if (!visited.has(depId)) {
      visited.add(depId);
      toLoad.push(depId);
    }
  }
}
