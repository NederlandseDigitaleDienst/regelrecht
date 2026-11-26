/**
 * useHotspots - the articles of a traject where enrichment would help most
 * (`GET /api/trajects/{ref}/hotspots`), for the "Kan verrijking gebruiken"
 * block in the library sidebar and the marks in a law's article list.
 *
 * Loads when its scope changes and keeps the last good answer on a failure,
 * like the task list: a hotspot list that blinks away on one failed request
 * says less than one that is a minute old. No poll: the scores move with the
 * corpus, not with the clock, and the library reloads on every traject switch.
 */
import { ref, toValue, watch } from 'vue';
import type { MaybeRefOrGetter } from 'vue';
import { apiFetchJson } from '../lib/apiFetch';
import { hotspotsUrl } from '../lib/enrichChain';
import type { Hotspots } from '../lib/enrichChain';

export interface HotspotsScope {
  trajectRef: MaybeRefOrGetter<string | null | undefined>;
  /** Only the hotspots of this law. */
  lawId?: MaybeRefOrGetter<string | null | undefined>;
  /** Off: nothing is loaded and the answer is empty (flag `enrich.hotspots`). */
  enabled: MaybeRefOrGetter<boolean>;
  limit?: number;
}

const EMPTY: Hotspots = { hotspots: [], per_article: {}, partial: false };

export function useHotspots(scope: HotspotsScope) {
  const data = ref<Hotspots>(EMPTY);
  const loaded = ref(false);
  // Bumped per load; an answer for an older scope is dropped.
  let generation = 0;

  async function load(): Promise<void> {
    const trajectRef = toValue(scope.trajectRef);
    const lawId = scope.lawId ? toValue(scope.lawId) : null;
    const mine = ++generation;
    if (!toValue(scope.enabled) || !trajectRef || (scope.lawId && !lawId)) {
      data.value = EMPTY;
      loaded.value = false;
      return;
    }
    try {
      const result = await apiFetchJson<Hotspots>(hotspotsUrl(trajectRef, scope.limit ?? 5, lawId));
      if (mine !== generation) return;
      data.value = result;
      loaded.value = true;
    } catch {
      // Keep the last answer; the block is a pointer, not a report.
    }
  }

  watch(
    () => [toValue(scope.trajectRef), scope.lawId ? toValue(scope.lawId) : null, toValue(scope.enabled)],
    () => {
      void load();
    },
    { immediate: true },
  );

  return { hotspots: data, loaded, reload: load };
}
