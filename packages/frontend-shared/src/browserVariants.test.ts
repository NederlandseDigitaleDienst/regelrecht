import { describe, it, expect, beforeEach, vi } from 'vitest';

/**
 * A browser variant is the only thing a user makes in this demo that they
 * cannot simply get back. These tests pin that it survives a reload, that it
 * can never shadow a checked-in variant, and that it reports when the corpus
 * changed underneath it.
 */
function freshStorage() {
  const data = new Map<string, string>();
  return {
    getItem: (k: string) => (data.has(k) ? data.get(k)! : null),
    setItem: (k: string, v: unknown) => void data.set(k, String(v)),
    removeItem: (k: string) => void data.delete(k),
    get length() { return data.size; },
    key: (i: number) => [...data.keys()][i] ?? null,
    _data: data,
  };
}

let storage: ReturnType<typeof freshStorage>;

async function load() {
  vi.resetModules();
  const persisted = await import('./usePersistedState');
  persisted.setCase('testcase');
  return import('./browserVariants');
}

describe('browser variants', () => {
  beforeEach(() => {
    storage = freshStorage();
    vi.stubGlobal('localStorage', new Proxy(storage, {
      ownKeys: () => [...storage._data.keys()],
      getOwnPropertyDescriptor: () => ({ enumerable: true, configurable: true }),
    }));
    vi.stubGlobal('window', { location: { reload: vi.fn() } });
  });

  const file = (yamlText = 'value: 2', original = 'value: 1') => ({
    path: 'laws/wet/2025-01-01.yaml',
    yaml: yamlText,
    original,
  });

  it('saves a variant in the shape the lawStore knows', async () => {
    const { saveVariant } = await load();

    const v = saveVariant({ title: 'Voet naar 90%', files: [file()] });

    expect(v).toMatchObject({
      id: 'eigen-voet-naar-90',
      title: 'Voet naar 90%',
      origin: 'browser',
      files: [{ path: 'laws/wet/2025-01-01.yaml', base: 'laws/wet/2025-01-01.yaml' }],
    });
  });

  it('returns the saved YAML per file', async () => {
    const { saveVariant, browserVariantYaml } = await load();
    const v = saveVariant({ title: 'Test', files: [file('value: 42')] });

    expect(browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 42');
    expect(browserVariantYaml(v.id, 'laws/andere.yaml')).toBe(null);
  });

  it('survives a reload', async () => {
    const first = await load();
    first.saveVariant({ title: 'Blijft staan', files: [file()] });

    // A new module instance with the same localStorage: that is what a reload does.
    const again = await load();

    expect(again.browserVariants()).toHaveLength(1);
    expect(again.browserVariants()[0]?.title).toBe('Blijft staan');
  });

  it('stays when the user starts over', async () => {
    const { saveVariant } = await load();
    const persisted = await import('./usePersistedState');
    saveVariant({ title: 'Blijft', files: [file()] });
    persisted.writePersisted('kolommen', ['a1']);

    persisted.resetPersistedState();

    const again = await load();
    expect(again.browserVariants()).toHaveLength(1);
  });

  it('numbers a duplicate title instead of overwriting', async () => {
    const { saveVariant, browserVariants } = await load();

    const a = saveVariant({ title: 'Zelfde naam', files: [file('a: 1')] });
    const b = saveVariant({ title: 'Zelfde naam', files: [file('b: 2')] });

    expect(a.id).toBe('eigen-zelfde-naam');
    expect(b.id).toBe('eigen-zelfde-naam-2');
    expect(browserVariants()).toHaveLength(2);
  });

  // A browser variant called 'a1' would shadow the checked-in a1 from
  // variants.json, and the demo would then compute with something other than
  // the column header says.
  it('cannot shadow a checked-in variant', async () => {
    const { saveVariant, isBrowserVariant } = await load();

    const v = saveVariant({ title: 'a1', files: [file()], takenIds: ['a1', 'a2'] });

    expect(v.id).not.toBe('a1');
    expect(isBrowserVariant(v.id)).toBe(true);
    expect(isBrowserVariant('a1')).toBe(false);
  });

  it('refuses a variant without a title or without edits', async () => {
    const { saveVariant } = await load();

    expect(() => saveVariant({ title: '  ', files: [file()] })).toThrow(/titel/i);
    expect(() => saveVariant({ title: 'Leeg', files: [] })).toThrow(/niets bewerkt/i);
  });

  // A file's path must be a non-empty string. It used to be enough for it to be
  // truthy, which let a number or an object through into storage.
  it('refuses a file whose path is not a non-empty string, and stores nothing', async () => {
    const { saveVariant, browserVariants } = await load();

    for (const path of [42, { pad: 'laws/a.yaml' }, '', null, undefined]) {
      const files = [{ ...file(), path }] as unknown as Parameters<typeof saveVariant>[0]['files'];
      expect(() => saveVariant({ title: 'Raar pad', files })).toThrow(/Onbruikbaar bestand/);
    }
    expect(browserVariants()).toHaveLength(0);
    expect(storage.getItem('testcase:v1:varianten')).toBe(null);
  });

  it('deletes a variant', async () => {
    const { saveVariant, deleteVariant, browserVariants } = await load();
    const v = saveVariant({ title: 'Weg ermee', files: [file()] });

    expect(deleteVariant(v.id)).toBe(true);
    expect(browserVariants()).toHaveLength(0);
    expect(deleteVariant(v.id)).toBe(false);
  });

  describe('updating', () => {
    // Without this a variant could only grow: someone who kept working in their
    // own variant got a nearly identical one next to it on every save.
    it('overwrites the same variant instead of adding one next to it', async () => {
      const { saveVariant, updateVariant, browserVariants, browserVariantYaml } = await load();
      const v = saveVariant({ title: 'Mijn variant', files: [file('value: 2')] });

      const { variant: updated } = updateVariant(v.id, { files: [file('value: 3')] });

      expect(updated.id).toBe(v.id);
      expect(browserVariants()).toHaveLength(1);
      expect(browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 3');
    });

    it('keeps the title when no new one is given', async () => {
      const { saveVariant, updateVariant } = await load();
      const v = saveVariant({ title: 'Blijft heten', files: [file()] });

      expect(updateVariant(v.id, { files: [file('value: 9')] }).variant.title).toBe('Blijft heten');
    });

    it('renames when a title is given, keeping the id', async () => {
      const { saveVariant, updateVariant } = await load();
      const v = saveVariant({ title: 'Oude naam', files: [file()] });

      const { variant: updated } = updateVariant(v.id, { files: [file()], title: 'Nieuwe naam' });

      // The id stays: otherwise a saved column choice points at nothing.
      expect(updated.id).toBe(v.id);
      expect(updated.title).toBe('Nieuwe naam');
    });

    // Updating overwrites work that was already there. When activating it
    // fails afterwards, the previous content has to be able to come back;
    // otherwise a variant stays behind that never loads again.
    it('restores the previous content with restore()', async () => {
      const { saveVariant, updateVariant, browserVariantYaml, browserVariant } = await load();
      const v = saveVariant({ title: 'Herstel', files: [file('value: 2')] });
      const created = browserVariant(v.id)?.created;
      expect(created).toBeTruthy();

      const { restore } = updateVariant(v.id, { files: [file('value: 3')], title: 'Andere naam' });
      expect(browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 3');

      restore();

      const after = browserVariant(v.id);
      expect(after).not.toBeNull();
      expect(browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 2');
      expect(after?.title).toBe('Herstel');
      expect(after?.created).toBe(created);
      expect(after?.updated).toBeUndefined();
    });

    it('leaves no second variant behind after restore', async () => {
      const { saveVariant, updateVariant, browserVariants } = await load();
      const v = saveVariant({ title: 'Eén blijft', files: [file()] });

      updateVariant(v.id, { files: [file('value: 5')] }).restore();

      expect(browserVariants()).toHaveLength(1);
    });

    it('carries `updated` into the shape the app reads', async () => {
      const { saveVariant, updateVariant, browserVariants } = await load();
      const v = saveVariant({ title: 'Zichtbaar', files: [file()] });

      // Freshly saved: never updated.
      expect(browserVariants()[0]?.updated).toBe(null);

      updateVariant(v.id, { files: [file('value: 6')] });

      expect(browserVariants()[0]?.updated).toBeTruthy();
    });

    it('keeps `created` and records when it was updated', async () => {
      const { saveVariant, updateVariant, browserVariant } = await load();
      const v = saveVariant({ title: 'Tijden', files: [file()] });
      const created = browserVariant(v.id)?.created;
      expect(created).toBeTruthy();

      updateVariant(v.id, { files: [file('value: 4')] });

      const after = browserVariant(v.id);
      expect(after).not.toBeNull();
      expect(after?.created).toBe(created);
      expect(after?.updated).toBeTruthy();
    });

    it('refreshes the fingerprint, so drift starts from the new base', async () => {
      const { saveVariant, updateVariant, checkDrift } = await load();
      const v = saveVariant({ title: 'Drift', files: [file('value: 2', 'value: 1')] });

      // The law changed; saving again happens on that new text.
      updateVariant(v.id, { files: [file('value: 3', 'value: 1b')] });

      expect(checkDrift(v.id, () => 'value: 1b').drifted).toBe(false);
      expect(checkDrift(v.id, () => 'value: 1').drifted).toBe(true);
    });

    it('survives a reload', async () => {
      const first = await load();
      const v = first.saveVariant({ title: 'Na ververs', files: [file('value: 2')] });
      first.updateVariant(v.id, { files: [file('value: 7')] });

      const second = await load();
      expect(second.browserVariants()).toHaveLength(1);
      expect(second.browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 7');
    });

    it('refuses a variant that is not stored here', async () => {
      const { updateVariant } = await load();

      expect(() => updateVariant('eigen-bestaat-niet', { files: [file()] })).toThrow(/niet in deze browser/);
    });

    it('refuses an empty set of files', async () => {
      const { saveVariant, updateVariant } = await load();
      const v = saveVariant({ title: 'Leeg', files: [file()] });

      expect(() => updateVariant(v.id, { files: [] })).toThrow(/niets bewerkt/);
    });

    it('refuses a file whose path is not a string and keeps the stored content', async () => {
      const { saveVariant, updateVariant, browserVariantYaml } = await load();
      const v = saveVariant({ title: 'Blijft staan', files: [file('value: 2')] });

      for (const path of [42, { pad: 'laws/a.yaml' }, '']) {
        const files = [{ ...file('value: 9'), path }] as unknown as NonNullable<Parameters<typeof updateVariant>[1]>['files'];
        expect(() => updateVariant(v.id, { files })).toThrow(/Onbruikbaar bestand/);
      }
      expect(browserVariantYaml(v.id, 'laws/wet/2025-01-01.yaml')).toBe('value: 2');
    });
  });

  describe('drift', () => {
    it('reports nothing while the corpus stays the same', async () => {
      const { saveVariant, checkDrift } = await load();
      const v = saveVariant({ title: 'Stabiel', files: [file('value: 2', 'value: 1')] });

      expect(checkDrift(v.id, () => 'value: 1')).toEqual({
        drifted: false, paths: [], unknown: false,
      });
    });

    // The law changed underneath the variant: the edit was made on a text that
    // no longer reads that way.
    it('reports the file whose base changed', async () => {
      const { saveVariant, checkDrift } = await load();
      const v = saveVariant({ title: 'Afgedreven', files: [file('value: 2', 'value: 1')] });

      const result = checkDrift(v.id, () => 'value: 999');

      expect(result.drifted).toBe(true);
      expect(result.paths).toEqual(['laws/wet/2025-01-01.yaml']);
    });

    // A variant from before this check carries no original. Then there is
    // nothing to compare, and the app would rather say nothing than something
    // untrue.
    it('says unknown when the variant carries no starting point', async () => {
      const { saveVariant, checkDrift } = await load();
      const v = saveVariant({
        title: 'Oud',
        files: [{ path: 'laws/wet/2025-01-01.yaml', yaml: 'value: 2' }],
      });

      const result = checkDrift(v.id, () => 'value: 999');

      expect(result).toEqual({ drifted: false, paths: [], unknown: true });
    });

    it('skips a file the app does not know now', async () => {
      const { saveVariant, checkDrift } = await load();
      const v = saveVariant({ title: 'Verdwenen', files: [file()] });

      expect(checkDrift(v.id, () => undefined).drifted).toBe(false);
    });

    // The base text is not stored a second time: a law here is ~50 KB and
    // storage fills up around 5 MB.
    it('stores a fingerprint and not the whole base text', async () => {
      const { saveVariant, browserVariant } = await load();
      const large = `x: ${'y'.repeat(5000)}`;
      const v = saveVariant({
        title: 'Groot',
        files: [{ path: 'laws/a.yaml', yaml: 'bewerkt', original: large }],
      });

      const stored = browserVariant(v.id)?.files[0];
      // The type has no `original`; the test checks that none was stored.
      expect((stored as Record<string, unknown> | undefined)?.original).toBeUndefined();
      expect(stored?.originalHash?.length).toBeLessThan(20);
    });
  });

  describe('fingerprint', () => {
    it('differs as soon as the text changes', async () => {
      const { fingerprint } = await load();
      expect(fingerprint('value: 1')).toBe(fingerprint('value: 1'));
      expect(fingerprint('value: 1')).not.toBe(fingerprint('value: 2'));
      expect(fingerprint('ab')).not.toBe(fingerprint('ba'));
      expect(fingerprint(null)).toBe(null);
    });
  });

  // Variants saved before the code was translated carry Dutch field names.
  // They must keep working, and come back in the current format.
  describe('variants saved in the old format', () => {
    const key = 'testcase:v1:varianten';
    const oldFormat = (fingerprintOfBase: string | null) => [{
      id: 'eigen-oud-formaat',
      titel: 'Oud formaat',
      gemaakt: '2026-09-01T10:00:00.000Z',
      bijgewerkt: '2026-09-02T10:00:00.000Z',
      basis: 'a1',
      bestanden: [{ pad: 'laws/wet/2025-01-01.yaml', yaml: 'value: 2', origineelHash: fingerprintOfBase }],
    }];

    it('are read under their old field names', async () => {
      const { fingerprint } = await load();
      storage.setItem(key, JSON.stringify(oldFormat(fingerprint('value: 1'))));

      const { browserVariants, browserVariantYaml, checkDrift } = await load();

      expect(browserVariants()).toEqual([{
        id: 'eigen-oud-formaat',
        title: 'Oud formaat',
        files: [{ path: 'laws/wet/2025-01-01.yaml', base: 'laws/wet/2025-01-01.yaml' }],
        origin: 'browser',
        created: '2026-09-01T10:00:00.000Z',
        updated: '2026-09-02T10:00:00.000Z',
        base: 'a1',
      }]);
      expect(browserVariantYaml('eigen-oud-formaat', 'laws/wet/2025-01-01.yaml')).toBe('value: 2');
      expect(checkDrift('eigen-oud-formaat', () => 'value: 1')).toEqual({ drifted: false, paths: [], unknown: false });
      expect(checkDrift('eigen-oud-formaat', () => 'value: 9').drifted).toBe(true);
    });

    it('are written back in the current format', async () => {
      storage.setItem(key, JSON.stringify(oldFormat(null)));

      await load();

      const stored = JSON.parse(storage.getItem(key) ?? '[]');
      expect(stored).toEqual([{
        id: 'eigen-oud-formaat',
        title: 'Oud formaat',
        created: '2026-09-01T10:00:00.000Z',
        updated: '2026-09-02T10:00:00.000Z',
        base: 'a1',
        files: [{ path: 'laws/wet/2025-01-01.yaml', yaml: 'value: 2', originalHash: null }],
      }]);
    });
  });

  describe('loading', () => {
    const key = 'testcase:v1:varianten';
    const current = [{
      id: 'eigen-huidig', title: 'Huidig', created: '2026-10-01T10:00:00.000Z', updated: null, base: null,
      files: [{ path: 'laws/a.yaml', yaml: 'value: 1', originalHash: null }],
    }];

    // An import sort could load this module before the app calls setCase().
    // Storage then cannot be read, and the variants must not be overwritten
    // with the empty fallback once the case is set. Vitest awaits between
    // module bodies, so it cannot reproduce the browser's order exactly; this
    // checks that loading without a case writes nothing.
    it('leaves stored variants alone when it loads before setCase()', async () => {
      storage.setItem(key, JSON.stringify([{ id: 'eigen-oud', titel: 'Oud', gemaakt: 'x', bestanden: [] }]));
      vi.resetModules();
      const variants = await import('./browserVariants');
      (await import('./usePersistedState')).setCase('testcase');
      await new Promise((r) => setTimeout(r, 0));

      expect(JSON.parse(storage.getItem(key)!)).toEqual([{ id: 'eigen-oud', titel: 'Oud', gemaakt: 'x', bestanden: [] }]);
      expect(variants.browserVariants()).toEqual([]);
    });

    it('does not write when everything is already in the current format', async () => {
      const text = JSON.stringify(current);
      storage.setItem(key, text);

      const { browserVariants } = await load();
      await new Promise((r) => setTimeout(r, 0));

      expect(storage.getItem(key)).toBe(text);
      expect(browserVariants()).toHaveLength(1);
    });

    it('creates nothing for a user without variants', async () => {
      await load();
      await new Promise((r) => setTimeout(r, 0));

      expect(storage.getItem(key)).toBe(null);
    });

    it('skips records it cannot read instead of failing to load', async () => {
      storage.setItem(key, JSON.stringify([
        null,
        { titel: 'Zonder id', bestanden: [] },
        { id: 'eigen-raar', titel: 'Rare bestanden', gemaakt: 'x', bestanden: { pad: 'laws/a.yaml' } },
        { id: 'eigen-gat', titel: 'Met een gat', gemaakt: 'x', bestanden: [null, { pad: 'laws/a.yaml', yaml: 'v: 1' }] },
      ]));

      const { browserVariants, browserVariantYaml } = await load();

      expect(browserVariants().map((v) => v.id).sort()).toEqual(['eigen-gat', 'eigen-raar']);
      expect(browserVariantYaml('eigen-gat', 'laws/a.yaml')).toBe('v: 1');
      expect(browserVariantYaml('eigen-raar', 'laws/a.yaml')).toBe(null);
    });
  });

  // A "saved" that turns out to be gone after a reload is worse than a
  // refusal: the user then believes their work is safe.
  it('fails loudly when storage is full instead of silently', async () => {
    const { saveVariant, browserVariants } = await load();
    const realSet = storage.setItem;
    storage.setItem = () => { throw new DOMException('vol', 'QuotaExceededError'); };

    try {
      expect(() => saveVariant({ title: 'Past niet', files: [file()] }))
        .toThrow(/opslag zit vol|staat uit/i);
    } finally {
      storage.setItem = realSet;
    }

    expect(browserVariants()).toHaveLength(0);
  });
});
