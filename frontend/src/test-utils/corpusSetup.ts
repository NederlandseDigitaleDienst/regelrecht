// Installs an in-memory corpus reader before every test, so a component that
// reads the corpus never reaches the network. A test that needs ruleworks
// calls `installTestCorpus` with them.
import { beforeEach } from 'vitest';
import { ref, type Ref } from 'vue';
import { createCorpusReader, installCorpus, type CorpusReader } from '../corpus';
import { createMemoryTransport, type MemoryCorpus, type MemoryTransport } from '../corpus/memoryTransport';

export interface TestCorpus {
  corpus: CorpusReader;
  transport: MemoryTransport;
  /** The active traject; set it to switch, as a route change would. */
  trajectRef: Ref<string | null>;
}

/**
 * Install a reader over `corpora` (keyed by traject ref, `''` for the global
 * corpus), with `active` as the active traject.
 */
export function installTestCorpus(corpora: Record<string, MemoryCorpus> = { '': {} }, active: string | null = null): TestCorpus {
  const transport = createMemoryTransport(corpora);
  const trajectRef = ref<string | null>(active);
  const corpus = createCorpusReader({ transport, trajectRef });
  installCorpus(corpus);
  return { corpus, transport, trajectRef };
}

beforeEach(() => {
  installTestCorpus();
});
