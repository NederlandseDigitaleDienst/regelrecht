import type { LawListPage, LawListQuery, LawOutput, TrajectRef } from './types';

/**
 * The reads the corpus reader needs from the editor-api, one request each and
 * without caching. Two adapters fill it: `createHttpTransport` in the app and
 * `createMemoryTransport` in tests.
 *
 * Every method rejects with an `ApiError` (carrying the HTTP status) when the
 * read fails, so a caller of the reader can still tell a 404 from a 500.
 */
export interface CorpusTransport {
  /** The current version of a rulework: its YAML text and ETag. */
  law(trajectRef: TrajectRef, lawId: string): Promise<{ text: string; etag: string | null }>;
  /** Every version's YAML text, newest first. */
  versions(trajectRef: TrajectRef, lawId: string): Promise<string[]>;
  outputs(trajectRef: TrajectRef, lawId: string): Promise<LawOutput[]>;
  /** Ids of the ruleworks that implement an open term of `lawId`. */
  implementors(trajectRef: TrajectRef, lawId: string): Promise<string[]>;
  list(trajectRef: TrajectRef, query: LawListQuery): Promise<LawListPage>;
  /** Ids of the ruleworks edited on the traject's branch. */
  changedLaws(trajectRef: string): Promise<string[]>;
}
