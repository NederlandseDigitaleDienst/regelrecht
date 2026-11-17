/**
 * The feature files open in the editor, as a store the whole editor shares.
 *
 * One open copy per traject, regelwerk and file, so every view of a file reads
 * the same text and the same unsaved edits. The copies live in memory only; a
 * reload starts from the saved files. A view that edits a file decides how
 * long its edits live: the scenario pane throws them away when its edit sheet
 * closes. A copy without edits stays cached for the session, which costs one
 * parsed feature per file.
 */
import { shallowReactive, toRaw } from 'vue';
import { defineStore } from 'pinia';
import { createOpenScenario, scenarioFileKey, type OpenScenario, type ScenarioFileRef } from '../scenario/openScenario';

export const useOpenScenariosStore = defineStore('openScenarios', () => {
  const files = shallowReactive(new Map<string, OpenScenario>());

  /**
   * The open copy of a file, created (not yet loaded) on first use. Reading
   * it does not track the map: a screen that opened a file in a computed must
   * not open it again when the file is closed under it (its tab closing).
   */
  function open(file: ScenarioFileRef): OpenScenario {
    const key = scenarioFileKey(file);
    let doc = toRaw(files).get(key);
    if (!doc) {
      doc = createOpenScenario({ ...file });
      files.set(key, doc);
    }
    return doc;
  }

  /** The open copy of a file, if there is one. */
  function find(file: ScenarioFileRef): OpenScenario | undefined {
    return files.get(scenarioFileKey(file));
  }

  /** Forget a file, edits and all. */
  function close(file: ScenarioFileRef): void {
    files.delete(scenarioFileKey(file));
  }

  /** The files of a traject that hold unsaved edits. */
  function dirtyIn(trajectRef: string | null | undefined): OpenScenario[] {
    return [...files.values()].filter((doc) => doc.file.trajectRef === trajectRef && doc.dirty.value);
  }

  /** Whether any file anywhere holds unsaved edits. */
  function anyDirty(): boolean {
    return [...files.values()].some((doc) => doc.dirty.value);
  }

  /** Forget every file of a traject. */
  function closeTraject(trajectRef: string | null | undefined): void {
    for (const [key, doc] of files) {
      if (doc.file.trajectRef === trajectRef) files.delete(key);
    }
  }

  return { files, open, find, close, dirtyIn, anyDirty, closeTraject };
});
