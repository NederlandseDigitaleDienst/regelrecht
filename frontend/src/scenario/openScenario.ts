/**
 * One feature file open in the editor: its saved text, the scenarios parsed
 * from it, and the edits to them that are not saved yet.
 *
 * The edits are kept per scenario as the values its form shows
 * (`ScenarioValues`). A save writes them into the parsed feature the same way
 * a run reads them (`applyValues` / `engineInputs`), so what runs is what the
 * file will say. The ETag of the last read or save is sent back as `If-Match`,
 * so a concurrent edit by another traject member surfaces as a conflict
 * instead of being overwritten.
 *
 * A file is opened in a traject, because scenario writes only exist there.
 * Nothing here has a lifecycle hook: the store that keeps the open files
 * (`stores/openScenarios.ts`) decides how long one lives.
 */
import { computed, ref, shallowRef } from 'vue';
import { parseFeature } from '../gherkin/parser';
import { formStateToGherkin, mapFeatureToForm, type FormState } from '../gherkin/formMapper';
import { scenarioFileUrl } from '../composables/corpusUrls';
import { lastSavedPr, sanitizeSavedPr } from '../composables/useSavedPr';
import { apiFetch, type RequestError } from '../lib/apiFetch';
import { applyValues, engineInputs, initialValues, type EngineInputs, type ScenarioValues } from './inputs';

/** Which feature file: a file in a regelwerk's scenario folder, in a traject. */
export interface ScenarioFileRef {
  trajectRef: string;
  lawId: string;
  filename: string;
}

/** The key a file is kept under; one open copy per traject, regelwerk and file. */
export function scenarioFileKey(file: ScenarioFileRef): string {
  return `${file.trajectRef}::${file.lawId}::${file.filename}`;
}

/** Shown when the file changed on the server since it was read. */
export const CONFLICT_MESSAGE =
  'Het scenario is intussen door iemand anders gewijzigd. ' +
  'Herlaad het scenario om de nieuwste versie te zien en voer ' +
  'je wijziging daarna opnieuw door.';

// Plain data, so a JSON round trip both copies and compares it.
function copy<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
function same(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function createOpenScenario(file: ScenarioFileRef) {
  const loading = ref(false);
  const loaded = ref(false);
  const error = ref<RequestError | null>(null);
  const saving = ref(false);
  const saveError = ref<RequestError | null>(null);
  /** The saved feature text, as read or as last written. */
  const featureText = ref<string | null>(null);
  /** The saved feature, parsed; null before a load or when it does not parse. */
  const state = shallowRef<FormState | null>(null);
  /** The values each scenario's form starts from. */
  const savedValues = shallowRef<ScenarioValues[]>([]);
  /** The values the forms show now: the edit buffer. */
  const values = ref<ScenarioValues[]>([]);
  let etag: string | null = null;
  // Each load or save bumps this; a response for an older one is dropped.
  let generation = 0;
  // Each load bumps this, and only the latest load clears `loading`: a save
  // that starts while a load runs drops the load's answer, not the flag.
  let loadToken = 0;

  const scenarios = computed(() => state.value?.scenarios ?? []);
  const featureName = computed(() => state.value?.featureName ?? '');
  /**
   * The names more than one scenario in the file carries, in file order. The
   * file runs and saves as it is (edits apply by position), but a link by name
   * reaches only the first; keeping names apart is the author's.
   */
  const duplicateNames = computed(() => {
    const seen = new Set<string>();
    const twice = new Set<string>();
    for (const s of scenarios.value) (seen.has(s.name) ? twice : seen).add(s.name);
    return [...twice];
  });

  /** The indexes of the scenarios whose values differ from the saved file. */
  const dirtyIndexes = computed(() =>
    values.value.flatMap((v, i) => (same(v, savedValues.value[i]) ? [] : [i])),
  );
  const dirty = computed(() => dirtyIndexes.value.length > 0);

  function adopt(text: string, newEtag: string | null): void {
    featureText.value = text;
    etag = newEtag;
    try {
      state.value = mapFeatureToForm(parseFeature(text));
    } catch {
      state.value = null;
    }
    savedValues.value = state.value ? initialValues(state.value) : [];
    values.value = copy(savedValues.value);
  }

  /**
   * Read the file. A loaded file is read again only with `fresh`, and never
   * over unsaved edits: those win until they are saved or discarded.
   */
  async function load({ fresh = false }: { fresh?: boolean } = {}): Promise<void> {
    if (loading.value || (loaded.value && !fresh) || dirty.value) return;
    const mine = ++generation;
    const myLoad = ++loadToken;
    loading.value = true;
    error.value = null;
    try {
      const res = await apiFetch(scenarioFileUrl(file.trajectRef, file.lawId, file.filename), {
        errorMessage: (status) =>
          status === 404 ? `Scenario '${file.filename}' is niet gevonden (${status})` : `Scenario laden mislukt: ${status}`,
      });
      const text = await res.text();
      if (mine !== generation) return;
      adopt(text, res.headers.get('ETag'));
      loaded.value = true;
    } catch (e) {
      if (mine !== generation) return;
      error.value = e as RequestError;
    } finally {
      if (myLoad === loadToken) loading.value = false;
    }
  }

  /** Replace scenario `index`'s values (a form's edit). */
  function setValues(index: number, next: ScenarioValues): void {
    if (index < 0 || index >= values.value.length) return;
    const updated = [...values.value];
    updated[index] = copy(next);
    values.value = updated;
  }

  /** Throw every unsaved edit away. */
  function discard(): void {
    values.value = copy(savedValues.value);
    saveError.value = null;
  }

  /**
   * Throw the edits away and read the file as it is on the server now: the way
   * out of a conflict, when someone else saved it in the meantime.
   */
  async function reload(): Promise<void> {
    discard();
    await load({ fresh: true });
  }

  /**
   * The engine inputs of scenario `index` as it stands now, edits included.
   * An untouched scenario runs from the file as read, as a save leaves it.
   */
  function inputsFor(index: number): EngineInputs | null {
    if (!state.value) return null;
    const edited = dirtyIndexes.value.includes(index) ? values.value[index] ?? null : null;
    return engineInputs(state.value, index, edited, file.lawId);
  }

  /** The scenario called `name`; the first one when no scenario has that name. */
  function indexOfName(name: string | null | undefined): number {
    if (scenarios.value.length === 0) return -1;
    const found = name ? scenarios.value.findIndex((s) => s.name === name) : -1;
    return found >= 0 ? found : 0;
  }

  /**
   * The feature text the edits would save. Only edited scenarios are written
   * in: an untouched one keeps its steps as read, so a save does not add, say,
   * today's date to a scenario that had none.
   */
  function editedText(): string | null {
    if (!state.value) return null;
    const edited = new Set(dirtyIndexes.value);
    return formStateToGherkin(applyValues(state.value, values.value.map((v, i) => (edited.has(i) ? v : null))));
  }

  /** Write the edits. Resolves to whether the file was saved. */
  async function save(): Promise<boolean> {
    const text = editedText();
    if (text === null || saving.value) return false;
    // What is being saved. The form stays open while the request runs; an edit
    // made in the meantime is not in this save and must survive it.
    const sent = copy(values.value);
    const mine = ++generation;
    saving.value = true;
    saveError.value = null;
    try {
      const headers: Record<string, string> = { 'Content-Type': 'text/plain; charset=utf-8' };
      if (etag) headers['If-Match'] = etag;
      const res = await apiFetch(scenarioFileUrl(file.trajectRef, file.lawId, file.filename), {
        method: 'PUT',
        headers,
        body: text,
        // 412 resolves (handled as a conflict below); other failures throw.
        allowStatuses: [412],
        errorMessage: (status, body) => body || `Opslaan mislukt: ${status}`,
      });
      if (res.status === 412) throw new Error(CONFLICT_MESSAGE);
      // Same `{ pr, etag }` shape as a law save: the PR feeds the shared
      // "PR #N" badge.
      let json: { pr?: unknown; etag?: string | null } | null = null;
      try {
        json = await res.json();
        lastSavedPr.value = sanitizeSavedPr(json?.pr);
      } catch {
        // An older deployment answered with a bare 200: keep the prior PR.
      }
      if (mine !== generation) return true;
      const typedSince = values.value;
      adopt(text, res.headers.get('ETag') ?? json?.etag ?? null);
      // Keep what was typed while the save ran, on top of the saved file.
      if (typedSince.length === values.value.length) {
        // Same length as typedSince, checked above, so index i exists.
        values.value = typedSince.map((v, i) => (same(v, sent[i]) ? values.value[i]! : copy(v)));
      }
      return true;
    } catch (e) {
      if (mine === generation) saveError.value = e as RequestError;
      return false;
    } finally {
      if (mine === generation) saving.value = false;
    }
  }

  return {
    file,
    loading,
    loaded,
    error,
    saving,
    saveError,
    featureText,
    featureName,
    duplicateNames,
    state,
    scenarios,
    values,
    dirty,
    dirtyIndexes,
    load,
    reload,
    setValues,
    discard,
    inputsFor,
    indexOfName,
    editedText,
    save,
  };
}

export type OpenScenario = ReturnType<typeof createOpenScenario>;
