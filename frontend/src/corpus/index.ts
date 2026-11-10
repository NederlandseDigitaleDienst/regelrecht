// The corpus as the editor reads it. The app installs one reader at start-up
// (main.ts), bound to the route's traject; tests install one over an
// in-memory corpus (test-utils/corpusSetup.ts).
import type { CorpusReader } from './reader';

export { createCorpusReader } from './reader';
export type { CorpusReader, CorpusReaderOptions, TrajectCorpusView } from './reader';
export { createHttpTransport } from './httpTransport';
export type { CorpusTransport } from './transport';
export type * from './types';

// Kept on globalThis rather than in a module variable: a test that mocks a
// module this one depends on gets its own instance of this module, and must
// still see the reader the test setup installed.
const INSTALLED = Symbol.for('regelrecht.editor.corpusReader');
type WithReader = typeof globalThis & { [INSTALLED]?: CorpusReader };

/** Make `reader` the one `useCorpus()` returns. */
export function installCorpus(reader: CorpusReader): void {
  (globalThis as WithReader)[INSTALLED] = reader;
}

/** The installed corpus reader. Throws when none is installed, so a missing setup fails loudly. */
export function useCorpus(): CorpusReader {
  const reader = (globalThis as WithReader)[INSTALLED];
  if (!reader) throw new Error('No corpus reader installed; call installCorpus() first');
  return reader;
}
