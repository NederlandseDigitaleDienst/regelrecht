// Gedeelde "Toevoegen aan traject"-logica (promote): één implementatie van de
// POST /promote-aanroep met per-wet busy-state, de al-in-traject-administratie
// en de 409-afhandeling. Gebruikt door zowel de AddLawSheet ("Regelwerk
// toevoegen"-flow) als de gewone zoekresultaten (SearchPopover) — geen tweede
// promote-implementatie.
//
// De al-in-traject-set wordt gevoed vanuit de traject-gefedereerde zoeklijst:
// source_priority 0 = de eigen schrijfbare traject-repo, dus niet (nogmaals)
// promoteerbaar. De backend blijft de autoriteit: een 409 op de promote
// markeert de wet alsnog als al-in-traject.
import { ref, type Ref } from 'vue';
import { lawPromoteUrl } from './corpusUrls';
import { apiFetchJson, ApiError } from '../lib/apiFetch';

export const PROMOTE_ERROR_TEXT =
  'Toevoegen aan het traject is mislukt. Probeer het opnieuw of neem contact op.';

type PromoteState = 'busy' | 'done' | null;
export type PromoteOutcome = 'done' | 'conflict' | 'error' | null;

/** The fields read from a traject-federated search entry. */
interface SearchLaw {
  law_id: string;
  source_priority?: number | null;
}

export function useLawPromote(activeTrajectRef: Readonly<Ref<string | null | undefined>>) {
  // Per law_id: 'busy' tijdens de promote-POST, 'done' na succes.
  const promoteState = ref<Record<string, PromoteState>>({});
  // Eén foutmelding voor de laatste mislukte promote (niet-409).
  const promoteError = ref<string | null>(null);
  // Law-ids die al in de traject-repo staan (source_priority 0, of 409 van de
  // backend): promoten is dan niet mogelijk.
  const inTrajectIds = ref(new Set<string>());

  /** Ververs de al-in-traject-set vanuit een traject-gefedereerde zoeklijst. */
  function setLawsFromSearch(laws: readonly SearchLaw[]): void {
    inTrajectIds.value = new Set(
      laws.filter((l) => l.source_priority === 0).map((l) => l.law_id),
    );
  }

  function isInTraject(lawId: string): boolean {
    return inTrajectIds.value.has(lawId);
  }

  function clearPromoteError() {
    promoteError.value = null;
  }

  /**
   * Kopieer de wet naar de traject-repo via de promote-endpoint.
   * Retourneert 'done' | 'conflict' | 'error', of null als de aanroep een
   * no-op was (geen traject, al bezig, of al in het traject).
   */
  async function promote(lawId: string): Promise<PromoteOutcome> {
    if (!activeTrajectRef.value) return null;
    if (promoteState.value[lawId] === 'busy' || isInTraject(lawId)) return null;
    promoteState.value = { ...promoteState.value, [lawId]: 'busy' };
    promoteError.value = null;
    try {
      await apiFetchJson(lawPromoteUrl(activeTrajectRef.value, lawId), { method: 'POST' });
      promoteState.value = { ...promoteState.value, [lawId]: 'done' };
      return 'done';
    } catch (e) {
      promoteState.value = { ...promoteState.value, [lawId]: null };
      if (e instanceof ApiError && e.status === 409) {
        // Backend is de autoriteit: markeer als al-in-traject.
        inTrajectIds.value = new Set([...inTrajectIds.value, lawId]);
        return 'conflict';
      }
      promoteError.value = PROMOTE_ERROR_TEXT;
      return 'error';
    }
  }

  return {
    promoteState,
    promoteError,
    inTrajectIds,
    setLawsFromSearch,
    isInTraject,
    clearPromoteError,
    promote,
  };
}
