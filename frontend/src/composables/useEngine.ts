/**
 * useEngine - singleton WasmEngine instance with lazy initialization.
 *
 * Provides the engine and helpers for loading laws and dependencies.
 */
import { ref } from 'vue';
import { apiFetchText } from '../lib/apiFetch';
import { useCorpus } from '../corpus';
import { createLruMap } from '../lib/lruMap';
import type { GherkinEngine } from '@regelrecht/frontend-shared/gherkin';

/**
 * The `WasmEngine` methods the editor calls. A local structural type: the
 * generated declarations in public/wasm/pkg/ are a gitignored build artifact,
 * so they cannot be imported in code that must type-check without them.
 * Results the engine hands back as `any` are `unknown` here.
 */
export interface EditorEngine extends GherkinEngine {
  loadLaw(yaml: string): string;
  unloadLaw(lawId: string): boolean;
  clearDataSources(): void;
  executeWithTrace(
    lawId: string,
    outputName: string,
    parameters: Record<string, unknown>,
    calculationDate: string,
  ): unknown;
  resolveNote(lawId: string, selector: unknown, validFrom?: string | null): unknown;
  resolveNotes(lawId: string, annotationsYaml: string, validFrom?: string | null): unknown;
}

/** The wasm-bindgen JS glue module, as far as it is used here. */
interface WasmModule {
  default(moduleOrPath: string): Promise<unknown>;
  WasmEngine: new () => EditorEngine;
}

let engineInstance: EditorEngine | null = null;
let initPromise: Promise<EditorEngine> | null = null;

// Per-law tracking of which traject scope a YAML was loaded under. The
// WASM engine itself only knows law id, so without this every scope
// would see whichever copy happened to be loaded first - switching
// trajects then runs scenarios against a stale dependency. Keyed by
// `lawId`, value is the `trajectRef || ''` the load used.
//
// LRU-capped. In practice `unloadAllLaws()` already clears the map on
// every traject switch, so the map only grows within one traject's
// session. The cap is a safety net for any future code path that
// loads a dependency without going through `unloadAllLaws` on scope
// change. Cap mirrors `lawCache`'s order of magnitude (`useLaw.js`).
const loadedScopes = createLruMap<string, string>(50, {
  onEvict: (lawId) => {
    // Drop the WASM-side copy alongside the tracking entry - without
    // this the engine retains the law in memory even though our
    // scope-tracking forgot it, defeating the whole "safety net" of
    // the cap. `hasLaw` guards an unloadLaw call that the engine
    // would otherwise panic on if the law is already gone.
    if (engineInstance?.hasLaw(lawId)) {
      engineInstance.unloadLaw(lawId);
    }
  },
});

const ready = ref(false);
const initError = ref<Error | null>(null);

async function initEngine(): Promise<EditorEngine> {
  if (engineInstance) return engineInstance;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    try {
      // WASM pkg lives in public/wasm/pkg/ and is served as a static asset.
      // Use fetch + blob URL to load the JS glue, avoiding Vite's restriction
      // on importing files from /public in source code.
      const jsText = await apiFetchText('/wasm/pkg/regelrecht_engine.js', {
        errorMessage: (status) => `Failed to fetch WASM JS glue: ${status}`,
      });
      const blob = new Blob([jsText], { type: 'application/javascript' });
      const blobUrl = URL.createObjectURL(blob);
      const wasm = (await import(/* @vite-ignore */ blobUrl)) as WasmModule;
      URL.revokeObjectURL(blobUrl);
      await wasm.default('/wasm/pkg/regelrecht_engine_bg.wasm');
      engineInstance = new wasm.WasmEngine();
      ready.value = true;
      return engineInstance;
    } catch (e) {
      // Everything on this path (fetch, import, WASM init) rejects with an Error.
      initError.value = e as Error;
      throw e;
    }
  })();

  return initPromise;
}

/**
 * Fetch a law's YAML through the corpus reader and load it into the engine.
 * The read goes through the active traject (read-your-writes for in-progress
 * edits), and the scope recorded is the traject the versions came from.
 *
 * Loads **every** version of the law (not just today's-best) so the
 * engine's date-aware resolution can pick the one in force on a given
 * calculation date - the same contract the scenario dependency walker
 * uses. This keeps a single "load a law into the engine" semantic: any
 * caller (notes, gherkin steps) that brings a law into the shared engine
 * brings its full version set, so the `engine.hasLaw` skip guard never
 * leaves a law with only one (possibly future-dated) version loaded.
 *
 * If the law was previously loaded under a *different* scope, unload the
 * stale copy first (`unloadLaw` drops all of its versions) - otherwise
 * scenario runs after a traject switch would evaluate against the
 * previous scope's dependencies.
 */
async function loadDependency(lawId: string): Promise<void> {
  const engine = await initEngine();
  const corpus = useCorpus();
  if (engine.hasLaw(lawId)) {
    if (loadedScopes.get(lawId) === (corpus.trajectRef ?? '')) return;
    engine.unloadLaw(lawId);
  }

  const { trajectRef, yamls } = await corpus.versions(lawId);
  // Only record the scope once a body actually loaded, so an empty result
  // (unknown law) - or one whose every version was unloadable - stays
  // retryable rather than masking the law as "loaded under this scope".
  // The scope is the traject the versions came from, not the one active now.
  if (loadLawVersions(engine, yamls, lawId)) {
    loadedScopes.set(lawId, trajectRef ?? '');
  }
}

/**
 * Load a law's full version set into the engine, **isolating each version**.
 *
 * A single unloadable version must not prevent the engine from loading the
 * law's other, executable versions: a federated corpus routinely carries
 * versions the WASM engine can't parse (an older-schema or text-only
 * *harvested* version alongside a small executable one). Without isolation,
 * `engine.loadLaw` throwing on one version would abort the whole law and the
 * resolver would report "Law not found" even though a usable version existed.
 * The engine's date-aware `select_in` then picks the in-force version among
 * whatever loaded.
 *
 * @param {object} engine - WasmEngine instance
 * @param {string[]} yamls - version YAMLs (any/empty tolerated)
 * @param {string} lawId - for diagnostics
 * @returns {boolean} true if at least one version loaded
 */
export function loadLawVersions(
  // Only `loadLaw` is called, and its return value is not used.
  engine: { loadLaw(yaml: string): unknown },
  yamls: unknown,
  lawId: string,
): boolean {
  let anyLoaded = false;
  for (const versionYaml of (Array.isArray(yamls) ? yamls : []) as string[]) {
    try {
      engine.loadLaw(versionYaml);
      anyLoaded = true;
    } catch (e) {
      // A plain first argument: a law id is user-controlled text and must never
      // act as a console format string.
      console.warn('Skipped an unloadable version of a law:', lawId, e);
    }
  }
  return anyLoaded;
}

/**
 * Load a law YAML string into the engine and remember which scope it
 * came from. Callers that pass a raw YAML body (i.e. an edited law
 * being saved through the editor) should pass the scope they fetched
 * it under so a later `loadDependency` call doesn't think a copy from
 * another scope is still valid.
 *
 * Re-loads (same `lawId`) unload the previous copy first so the
 * engine sees the new YAML - `engine.loadLaw` on a known id is a no-op
 * in the WASM binding.
 */
async function loadLawYaml(
  yaml: string,
  lawId: string | null = null,
  trajectRef: string | null = null,
): Promise<string> {
  const engine = await initEngine();
  if (lawId && engine.hasLaw(lawId)) engine.unloadLaw(lawId);
  const result = engine.loadLaw(yaml);
  if (lawId) {
    loadedScopes.set(lawId, trajectRef || '');
  }
  return result;
}

/**
 * Unload a law from the engine and forget its scope. Used when a
 * traject switch needs to flush a known dependency so the next
 * resolver call re-fetches from the new scope.
 */
function unloadLaw(lawId: string): void {
  if (!engineInstance) return;
  if (engineInstance.hasLaw(lawId)) engineInstance.unloadLaw(lawId);
  loadedScopes.delete(lawId);
}

/**
 * Unload every law the engine has tracked. Cheaper than rebuilding
 * the engine on every traject switch in the editor, and good enough
 * because the dependency walker re-loads on demand.
 */
function unloadAllLaws(): void {
  if (!engineInstance) {
    loadedScopes.clear();
    return;
  }
  for (const lawId of loadedScopes.keys()) {
    if (engineInstance.hasLaw(lawId)) engineInstance.unloadLaw(lawId);
  }
  loadedScopes.clear();
}

export function useEngine() {
  return {
    /** Ref<boolean> - true when the WASM engine is ready */
    ready,
    /** Ref<Error|null> - set if init failed */
    initError,
    /** Initialize and return the engine instance */
    initEngine,
    /** Load a dependent law by ID via the API */
    loadDependency,
    /** Load raw YAML into the engine */
    loadLawYaml,
    /** Unload a single law from the engine */
    unloadLaw,
    /** Unload every tracked law - used on traject switch to flush stale deps */
    unloadAllLaws,
    /** Get the engine instance (must be initialized first) */
    getEngine: (): EditorEngine | null => engineInstance,
  };
}
