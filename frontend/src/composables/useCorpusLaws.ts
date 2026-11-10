import { useCorpus } from '../corpus';
export type { CorpusLaw } from '../corpus';

/**
 * Display names of ruleworks in the active traject.
 *
 * Kept while callers move to `useCorpus().displayName`; the corpus reader
 * now owns the names list and its cache. The argument is ignored: every
 * caller passed the route's traject, which the reader follows itself.
 */
export function useCorpusLaws(_trajectRef?: unknown) {
  const corpus = useCorpus();
  return { displayName: (lawId: string) => corpus.displayName(lawId) };
}
