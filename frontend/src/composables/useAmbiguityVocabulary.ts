/**
 * useAmbiguityVocabulary - the controlled tag list for questioning notes.
 *
 * RFC-018 Decision 9: a questioning note over an open norm carries a tagging
 * body whose value is one of these ids. The list is deliberately *not* a JSON
 * Schema enum (it is still emerging from interpretation research); `just
 * validate-annotations` only warns on an unknown value. The same file is
 * copied to /data/annotations/_vocabulary/ by copy-laws.js so the editor's
 * picker and the CI check read one source and cannot drift.
 */
import { ref } from 'vue';
import * as yaml from 'js-yaml';
import { apiFetchText } from '../lib/apiFetch';

// One entry of `ambiguity:` in the vocabulary YAML
// (corpus/annotations/_vocabulary/ambiguity.yaml). Not validated at runtime.
export interface VocabularyItem {
  id: string;
  label: string;
  description?: string;
}

// Session cache: the file does not change while the editor is open.
let cached: VocabularyItem[] | null = null;

const items = ref<VocabularyItem[]>([]);
const loaded = ref(false);

async function load(): Promise<VocabularyItem[] | null> {
  if (cached) {
    items.value = cached;
    loaded.value = true;
    return cached;
  }
  try {
    const text = await apiFetchText('/data/annotations/_vocabulary/ambiguity.yaml', {
      errorMessage: (status) => `HTTP ${status}`,
    });
    const doc = yaml.load(text) as { ambiguity?: unknown } | null | undefined;
    const list: VocabularyItem[] = Array.isArray(doc?.ambiguity) ? doc.ambiguity : [];
    cached = list;
    items.value = list;
  } catch (e) {
    // A missing vocabulary must not block note creation: the tag field just
    // falls back to free text. Surfaced via the empty list.
    console.warn('Kon ambiguïteit-vocabulary niet laden:', (e as Error).message);
    cached = [];
    items.value = [];
  } finally {
    loaded.value = true;
  }
  return cached;
}

// loaded.value stays false until load()'s finally settles, so two components
// mounting in the same tick would each dispatch a fetch. Hold the in-flight
// promise so the second caller reuses the first load.
let loadPromise: Promise<VocabularyItem[] | null> | null = null;
export function useAmbiguityVocabulary() {
  if (!loaded.value && !loadPromise) {
    loadPromise = load().finally(() => {
      loadPromise = null;
    });
  }
  return { items, loaded, reload: load };
}
