import { describe, it, expect, vi } from 'vitest';
import { computed, ref } from 'vue';
import { createCorpusReader } from './reader';
import { createMemoryTransport, type MemoryCorpus, type MemoryTransport } from './memoryTransport';

const TRAJECT = 'proef-0123abcd';
const OTHER = 'ander-89abcdef';

function lawYaml(id: string, name = `Naam van ${id}`): string {
  return `$id: ${id}\nname: ${name}\narticles:\n  - number: '1'\n    text: Tekst\n`;
}

/** The corpus of one traject in a test transport; fails the test when it is missing. */
function corpusOf(transport: MemoryTransport, trajectRef: string): MemoryCorpus & { laws: NonNullable<MemoryCorpus['laws']> } {
  const corpus = transport.corpora[trajectRef];
  if (!corpus) throw new Error(`no corpus for ${trajectRef}`);
  corpus.laws ??= {};
  return corpus as MemoryCorpus & { laws: NonNullable<MemoryCorpus['laws']> };
}

/** Hold the next call of a transport read until `release` is called. */
function holdNext<K extends 'law' | 'versions' | 'list'>(transport: MemoryTransport, read: K) {
  const original = transport[read].bind(transport) as (...args: unknown[]) => Promise<unknown>;
  let release!: () => void;
  const gate = new Promise<void>((r) => { release = r; });
  let held = false;
  (transport as unknown as Record<string, unknown>)[read] = async (...args: unknown[]) => {
    if (held) return original(...args);
    held = true;
    // Read the corpus now, answer once released: the answer is from before.
    const answer = await original(...args);
    await gate;
    return answer;
  };
  return () => release();
}

function setup(corpora: Record<string, MemoryCorpus>, active: string | null = null) {
  const transport = createMemoryTransport(corpora);
  const trajectRef = ref<string | null>(active);
  const corpus = createCorpusReader({ transport, trajectRef });
  return { transport, trajectRef, corpus };
}

describe('corpus reader', () => {
  describe('the active traject', () => {
    it('reads from the traject the route names at call time', async () => {
      const { corpus, trajectRef } = setup({
        '': { laws: { wet_a: lawYaml('wet_a', 'Globaal') } },
        [TRAJECT]: { laws: { wet_a: lawYaml('wet_a', 'In het traject') } },
      });

      expect((await corpus.law('wet_a')).lawName).toBe('Globaal');
      trajectRef.value = TRAJECT;
      expect((await corpus.law('wet_a')).lawName).toBe('In het traject');
      expect(corpus.trajectRef).toBe(TRAJECT);
    });

    it('treats an empty traject ref as the global corpus', async () => {
      const { corpus, trajectRef } = setup({ '': { laws: { wet_a: lawYaml('wet_a') } } });
      trajectRef.value = '';
      expect(corpus.trajectRef).toBeNull();
      expect((await corpus.law('wet_a')).law?.$id).toBe('wet_a');
    });

    it('reads a fixed traject through forTraject, whatever the route says', async () => {
      const { corpus, transport } = setup(
        { [OTHER]: { laws: { wet_a: lawYaml('wet_a', 'Ander') } } },
        TRAJECT,
      );

      const view = corpus.forTraject(OTHER);

      expect(view.trajectRef).toBe(OTHER);
      expect((await view.law('wet_a')).lawName).toBe('Ander');
      expect(transport.calls).toEqual([`${OTHER} law wet_a`]);
    });

    it('lets a read still running at a traject switch land in its own traject', async () => {
      const { corpus, transport, trajectRef } = setup({
        [TRAJECT]: { laws: { wet_a: lawYaml('wet_a', 'Eerste') } },
        [OTHER]: { laws: { wet_a: lawYaml('wet_a', 'Tweede') } },
      }, TRAJECT);
      const release = holdNext(transport, 'law');
      const pending = corpus.law('wet_a');

      trajectRef.value = OTHER;
      release();

      expect((await pending).lawName).toBe('Eerste');
      expect((await corpus.law('wet_a')).lawName).toBe('Tweede');
      expect((await corpus.forTraject(TRAJECT).law('wet_a')).lawName).toBe('Eerste');
      expect(transport.calls).toEqual([`${TRAJECT} law wet_a`, `${OTHER} law wet_a`]);
    });
  });

  describe('law', () => {
    it('serves a second read from the cache, per traject', async () => {
      const { corpus, transport, trajectRef } = setup({
        [TRAJECT]: { laws: { wet_a: lawYaml('wet_a') } },
        [OTHER]: { laws: { wet_a: lawYaml('wet_a') } },
      }, TRAJECT);

      await corpus.law('wet_a');
      await corpus.law('wet_a');
      trajectRef.value = OTHER;
      await corpus.law('wet_a');

      expect(transport.calls).toEqual([`${TRAJECT} law wet_a`, `${OTHER} law wet_a`]);
    });

    it('fetches again when asked for a fresh copy, and caches that', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { laws: { wet_a: lawYaml('wet_a', 'Oud') } } }, TRAJECT);
      await corpus.law('wet_a');
      corpusOf(transport, TRAJECT).laws.wet_a = lawYaml('wet_a', 'Nieuw');

      expect((await corpus.law('wet_a', { fresh: true })).lawName).toBe('Nieuw');
      expect((await corpus.law('wet_a')).lawName).toBe('Nieuw');
      expect(transport.calls).toHaveLength(2);
    });

    it('shares one request between concurrent reads', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { laws: { wet_a: lawYaml('wet_a') } } }, TRAJECT);

      const [a, b] = await Promise.all([corpus.law('wet_a'), corpus.law('wet_a', { fresh: true })]);

      expect(a).toBe(b);
      expect(transport.calls).toHaveLength(1);
    });

    it('returns the ETag with the body', async () => {
      const { corpus } = setup({ [TRAJECT]: { laws: { wet_a: lawYaml('wet_a') }, etags: { wet_a: '"v1"' } } }, TRAJECT);
      expect((await corpus.law('wet_a')).etag).toBe('"v1"');
    });

    it('also caches a body under its $id when opened by another id', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { laws: { alias: lawYaml('wet_echt') } } }, TRAJECT);

      await corpus.law('alias');
      const body = await corpus.law('wet_echt');

      expect(body.law?.$id).toBe('wet_echt');
      expect(transport.calls).toEqual([`${TRAJECT} law alias`]);
    });

    it('rejects with the status of a failed read, and retries on the next call', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { laws: {} } }, TRAJECT);

      await expect(corpus.law('wet_onbekend')).rejects.toMatchObject({ status: 404 });
      corpusOf(transport, TRAJECT).laws.wet_onbekend = lawYaml('wet_onbekend');
      expect((await corpus.law('wet_onbekend')).law?.$id).toBe('wet_onbekend');
    });

    it('resolves a name given as a reference to an action output', async () => {
      const text = [
        '$id: wet_ref',
        'name: "#naam"',
        'articles:',
        '  - number: "1"',
        '    machine_readable:',
        '      execution:',
        '        actions:',
        '          - output: naam',
        '            value: Wet met een verwezen naam',
      ].join('\n');
      const { corpus } = setup({ [TRAJECT]: { laws: { wet_ref: text } } }, TRAJECT);
      expect((await corpus.law('wet_ref')).lawName).toBe('Wet met een verwezen naam');
    });
  });

  describe('recordBody', () => {
    it('replaces the cached body of the traject it was recorded for', async () => {
      const { corpus, transport } = setup({
        [TRAJECT]: { laws: { wet_a: lawYaml('wet_a', 'Oud') } },
        [OTHER]: { laws: { wet_a: lawYaml('wet_a', 'Ander') } },
      }, OTHER);
      await corpus.forTraject(TRAJECT).law('wet_a');

      corpus.forTraject(TRAJECT).recordBody('wet_a', lawYaml('wet_a', 'Opgeslagen'), '"v2"');

      const body = await corpus.forTraject(TRAJECT).law('wet_a');
      expect(body).toMatchObject({ lawName: 'Opgeslagen', etag: '"v2"' });
      expect((await corpus.law('wet_a')).lawName).toBe('Ander');
      expect(transport.calls).toEqual([`${TRAJECT} law wet_a`, `${OTHER} law wet_a`]);
    });

    it('drops the cached law lists of that traject', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { laws: { wet_a: lawYaml('wet_a') } } }, TRAJECT);
      await corpus.allLaws();

      corpus.recordBody('wet_b', lawYaml('wet_b'), null);
      corpusOf(transport, TRAJECT).laws.wet_b = lawYaml('wet_b');

      expect((await corpus.allLaws()).map((l) => l.law_id)).toEqual(['wet_a', 'wet_b']);
    });

    it('drops the cached versions of that rulework, so the engine reloads the saved one', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { laws: { wet_a: lawYaml('wet_a', 'Oud') } } }, TRAJECT);
      await corpus.versions('wet_a');

      corpusOf(transport, TRAJECT).laws.wet_a = lawYaml('wet_a', 'Opgeslagen');
      corpus.recordBody('wet_a', lawYaml('wet_a', 'Opgeslagen'), null);

      expect((await corpus.versions('wet_a')).yamls).toEqual([lawYaml('wet_a', 'Opgeslagen')]);
    });

    it('is not overwritten by a read that started before it', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { laws: { wet_a: lawYaml('wet_a', 'Oud') } } }, TRAJECT);
      const release = holdNext(transport, 'law');
      const earlier = corpus.law('wet_a', { fresh: true });
      await vi.waitFor(() => expect(transport.calls).toHaveLength(1));

      corpusOf(transport, TRAJECT).laws.wet_a = lawYaml('wet_a', 'Opgeslagen');
      corpus.recordBody('wet_a', lawYaml('wet_a', 'Opgeslagen'), '"v2"');
      // A fresh read after the save does not join the read from before it.
      const later = corpus.law('wet_a', { fresh: true });
      release();

      expect((await earlier).lawName).toBe('Oud');
      expect((await later).lawName).toBe('Opgeslagen');
      expect((await corpus.law('wet_a')).lawName).toBe('Opgeslagen');
    });

    it('keeps showing the old names while the list is fetched again', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { list: [{ law_id: 'wet_a', name: 'Oude naam' }] } }, TRAJECT);
      const name = computed(() => corpus.displayName('wet_a'));
      await vi.waitFor(() => expect(name.value).toBe('Oude naam'));

      corpusOf(transport, TRAJECT).list = [{ law_id: 'wet_a', name: 'Nieuwe naam' }];
      corpus.recordBody('wet_a', lawYaml('wet_a', 'Nieuwe naam'), null);

      expect(name.value).toBe('Oude naam');
      await vi.waitFor(() => expect(name.value).toBe('Nieuwe naam'));
    });
  });

  describe('versions', () => {
    it('reports the traject it read from', async () => {
      const { corpus } = setup({ [TRAJECT]: { laws: { wet_a: [lawYaml('wet_a'), lawYaml('wet_a')] } } }, TRAJECT);

      const versions = await corpus.versions('wet_a');

      expect(versions.trajectRef).toBe(TRAJECT);
      expect(versions.yamls).toHaveLength(2);
    });

    it('caches a non-empty list but not an empty one', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { laws: { wet_a: lawYaml('wet_a') } } }, TRAJECT);

      await corpus.versions('wet_a');
      await corpus.versions('wet_a');
      expect(await corpus.versions('wet_nog_niet')).toMatchObject({ yamls: [] });
      corpusOf(transport, TRAJECT).laws.wet_nog_niet = lawYaml('wet_nog_niet');
      expect((await corpus.versions('wet_nog_niet')).yamls).toHaveLength(1);

      expect(transport.calls).toEqual([
        `${TRAJECT} versions wet_a`,
        `${TRAJECT} versions wet_nog_niet`,
        `${TRAJECT} versions wet_nog_niet`,
      ]);
    });
  });

  describe('outputs and implementors', () => {
    it('reads them from the active traject without caching', async () => {
      const outputs = [{ name: 'leeftijd', output_type: 'number' }];
      const { corpus, transport } = setup({
        [TRAJECT]: { laws: {}, outputs: { eigen_regeling: outputs }, implementors: { wet_a: ['regeling_b'] } },
      }, TRAJECT);

      expect(await corpus.outputs('eigen_regeling')).toEqual(outputs);
      expect(await corpus.outputs('eigen_regeling')).toEqual(outputs);
      expect(await corpus.implementors('wet_a')).toEqual(['regeling_b']);
      expect(transport.calls).toHaveLength(3);
    });
  });

  describe('law lists', () => {
    const list = [
      { law_id: 'wet_a', name: 'Wet A' },
      { law_id: 'wet_b', name: 'Wet B' },
    ];

    it('searches without caching and reports the total before the limit', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { list } }, TRAJECT);

      expect(await corpus.searchLaws({ q: 'wet', limit: 1 })).toEqual({ laws: [list[0]], total: 2 });
      await corpus.searchLaws({ q: 'wet', limit: 1 });
      expect(transport.calls).toHaveLength(2);
    });

    it('caches allLaws per traject', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { list } }, TRAJECT);
      await corpus.allLaws();
      await corpus.allLaws();
      expect(transport.calls).toEqual([`${TRAJECT} list`]);
    });

    it('does not cache a failed allLaws', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { list, failures: { list: 502 } } }, TRAJECT);
      await expect(corpus.allLaws()).rejects.toMatchObject({ status: 502 });
      delete corpusOf(transport, TRAJECT).failures;
      expect(await corpus.allLaws()).toEqual(list);
    });
  });

  describe('changedLaws', () => {
    it('is null outside a traject, without a request', async () => {
      const { corpus, transport } = setup({ '': {} });
      expect(await corpus.changedLaws()).toBeNull();
      expect(transport.calls).toEqual([]);
    });

    it('lists the edited ruleworks of the active traject', async () => {
      const { corpus } = setup({ [TRAJECT]: { changedLaws: ['wet_a'] } }, TRAJECT);
      expect(await corpus.changedLaws()).toEqual(['wet_a']);
    });
  });

  describe('displayName', () => {
    it('falls back to the readable id, then shows the name once the list arrives', async () => {
      const { corpus } = setup({ [TRAJECT]: { list: [{ law_id: 'wet_a', display_name: 'Wet over A' }] } }, TRAJECT);
      const name = computed(() => corpus.displayName('wet_a'));

      expect(name.value).toBe('Wet A');
      await vi.waitFor(() => expect(name.value).toBe('Wet over A'));
    });

    it('follows the traject switch', async () => {
      const { corpus, trajectRef } = setup({
        [TRAJECT]: { list: [{ law_id: 'wet_a', name: 'Eerste' }] },
        [OTHER]: { list: [{ law_id: 'wet_a', name: 'Tweede' }] },
      }, TRAJECT);
      const name = computed(() => corpus.displayName('wet_a'));

      await vi.waitFor(() => expect(name.value).toBe('Eerste'));
      trajectRef.value = OTHER;
      await vi.waitFor(() => expect(name.value).toBe('Tweede'));
    });

    it('asks for the names list once per traject', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { list: [{ law_id: 'wet_a', name: 'Wet A' }] } }, TRAJECT);

      corpus.displayName('wet_a');
      corpus.displayName('wet_b');
      await vi.waitFor(() => expect(corpus.displayName('wet_a')).toBe('Wet A'));
      corpus.displayName('wet_a');

      expect(transport.calls).toEqual([`${TRAJECT} list`]);
    });

    it('asks again for a failed list once the retry delay has passed', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { list: [{ law_id: 'wet_a', name: 'Wet over A' }], failures: { list: 500 } } }, TRAJECT);
      const now = vi.spyOn(Date, 'now').mockReturnValue(1_000_000);
      corpus.displayName('wet_a');
      await vi.waitFor(() => expect(transport.calls).toHaveLength(1));
      delete corpusOf(transport, TRAJECT).failures;

      now.mockReturnValue(1_000_000 + 10_001);
      corpus.displayName('wet_a');

      await vi.waitFor(() => expect(corpus.displayName('wet_a')).toBe('Wet over A'));
      expect(transport.calls).toHaveLength(2);
      now.mockRestore();
    });

    it('keeps the readable id after a failed list, without asking again right away', async () => {
      const { corpus, transport } = setup({ [TRAJECT]: { failures: { list: 500 } } }, TRAJECT);

      corpus.displayName('wet_a');
      await vi.waitFor(() => expect(transport.calls).toHaveLength(1));
      await Promise.resolve();

      expect(corpus.displayName('wet_a')).toBe('Wet A');
      expect(transport.calls).toHaveLength(1);
    });
  });
});
