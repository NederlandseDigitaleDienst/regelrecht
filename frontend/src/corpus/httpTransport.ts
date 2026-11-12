import { apiFetch, apiFetchJson } from '../lib/apiFetch';
import { corpusBase, lawUrl } from '../composables/corpusUrls';
import type { CorpusTransport } from './transport';
import type { CorpusLaw, LawListQuery, LawOutput, TrajectRef } from './types';

function listQueryString(query: LawListQuery): string {
  const parts: string[] = [];
  if (query.q !== undefined) parts.push(`q=${encodeURIComponent(query.q)}`);
  if (query.ids) parts.push(`ids=${encodeURIComponent(query.ids.join(','))}`);
  if (query.limit !== undefined) parts.push(`limit=${query.limit}`);
  if (query.loaded) parts.push('loaded=true');
  return parts.length > 0 ? `?${parts.join('&')}` : '';
}

/** The corpus reads over HTTP, against the global or the traject routes of the editor-api. */
export function createHttpTransport(): CorpusTransport {
  return {
    async law(trajectRef: TrajectRef, lawId: string) {
      const res = await apiFetch(lawUrl(trajectRef, lawId), {
        errorMessage: (status) => `Failed to fetch: ${status}`,
      });
      return { text: await res.text(), etag: res.headers.get('ETag') };
    },

    async versions(trajectRef: TrajectRef, lawId: string) {
      const yamls = await apiFetchJson<unknown>(`${lawUrl(trajectRef, lawId)}/versions`, {
        errorMessage: (status) => `Failed to fetch versions of law '${lawId}': ${status}`,
      });
      return Array.isArray(yamls) ? (yamls as string[]) : [];
    },

    async outputs(trajectRef: TrajectRef, lawId: string) {
      const outputs = await apiFetchJson<unknown>(`${lawUrl(trajectRef, lawId)}/outputs`);
      return Array.isArray(outputs) ? (outputs as LawOutput[]) : [];
    },

    async implementors(trajectRef: TrajectRef, lawId: string) {
      const ids = await apiFetchJson<unknown>(`${lawUrl(trajectRef, lawId)}/implementors`);
      return Array.isArray(ids) ? ids.filter((id): id is string => typeof id === 'string') : [];
    },

    async list(trajectRef: TrajectRef, query: LawListQuery) {
      const res = await apiFetch(`${corpusBase(trajectRef)}/laws${listQueryString(query)}`, {
        errorMessage: (status) => `Failed to load corpus: ${status}`,
      });
      const body: unknown = await res.json();
      const laws = Array.isArray(body) ? (body as CorpusLaw[]) : [];
      // The number of matches before the limit; an older backend omits it.
      const total = Number(res.headers.get('X-Total-Count')) || laws.length;
      return { laws, total };
    },

    async changedLaws(trajectRef: string) {
      const ids = await apiFetchJson<unknown>(`${corpusBase(trajectRef)}/changed-laws`);
      return Array.isArray(ids) ? ids.filter((id): id is string => typeof id === 'string') : [];
    },
  };
}
