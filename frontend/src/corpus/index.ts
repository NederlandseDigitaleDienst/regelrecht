// The corpus as the editor reads it. The app installs one reader at start-up
// (main.ts), bound to the route's traject; tests install one over an
// in-memory corpus (test-utils/corpusSetup.ts).
import type { CorpusReader } from './reader';

export { createCorpusReader } from './reader';
export type { CorpusReader, CorpusReaderOptions, TrajectCorpusView } from './reader';
export { createHttpTransport } from './httpTransport';
export type { CorpusTransport } from './transport';
export type * from './types';

let installed: CorpusReader | null = null;

/** Make `reader` the one `useCorpus()` returns. */
export function installCorpus(reader: CorpusReader): void {
  installed = reader;
}

/** The installed corpus reader. Throws when none is installed, so a missing setup fails loudly. */
export function useCorpus(): CorpusReader {
  if (!installed) throw new Error('No corpus reader installed; call installCorpus() first');
  return installed;
}
