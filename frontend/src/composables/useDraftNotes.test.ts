import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref } from 'vue';
import * as yaml from 'js-yaml';
import { useDraftNotes } from './useDraftNotes';
import { lastSavedPr } from './useSavedPr';
import type { Note } from './useNotes';
import { stubFetch } from '../test-utils/fetchStubs';

interface TestBody {
  type: string;
  value: string;
  purpose: string;
}

/** The test notes all carry a TextualBody. */
type TestNote = Note & { body: TestBody };

/** The parsed export document, as far as these tests read it. */
interface ExportDoc {
  $schema: string;
  annotations: TestNote[];
}

function loadExport(text: string): ExportDoc {
  return yaml.load(text) as ExportDoc;
}

/** The request options the PUT is sent with, as these tests read them. */
interface SentInit {
  method: string;
  headers: Record<string, string>;
  body: string;
}

function fetchCalls(): [string, SentInit][] {
  return vi.mocked(globalThis.fetch).mock.calls as unknown as [string, SentInit][];
}

/** Stub the sidecar fetch exportYaml does. `committed` is the file's notes. */
function stubSidecar(committed: Note[] | null) {
  stubFetch(vi.fn().mockResolvedValue(
    committed == null
      ? { ok: false, status: 404 }
      : {
          ok: true,
          text: async () => yaml.dump({ annotations: committed }),
        },
  ));
}

// useDraftNotes is the local-only authoring store (RFC-018 write path MVP):
// per-law localStorage, append/clear, and a committed+drafts YAML export.
// These pin the storage keying, the per-law isolation, and the export shape
// (the file a user commits by hand).

const NOTE: TestNote = {
  type: 'Annotation',
  motivation: 'commenting',
  target: {
    source: 'regelrecht://wet_op_de_zorgtoeslag',
    selector: { type: 'TextQuoteSelector', exact: 'normpremie' },
  },
  body: { type: 'TextualBody', value: 'uitleg', purpose: 'commenting' },
};

beforeEach(() => {
  localStorage.clear();
});

describe('useDraftNotes', () => {
  it('appends a draft and persists it under a per-law key', () => {
    const lawId = ref('wet_op_de_zorgtoeslag');
    const { addDraft, drafts } = useDraftNotes(lawId);
    addDraft(NOTE);
    expect(drafts.value).toHaveLength(1);
    const raw = localStorage.getItem('regelrecht-draft-notes:wet_op_de_zorgtoeslag');
    expect(JSON.parse(raw ?? 'null')).toHaveLength(1);
  });

  it('stamps created when the note has none', () => {
    const { addDraft } = useDraftNotes(ref('w'));
    const stored = addDraft(NOTE);
    expect(stored.created).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  });

  it('keeps an explicit created timestamp', () => {
    const { addDraft } = useDraftNotes(ref('w'));
    const stored = addDraft({ ...NOTE, created: '2020-01-01T00:00:00Z' });
    expect(stored.created).toBe('2020-01-01T00:00:00Z');
  });

  it('isolates drafts per law', () => {
    localStorage.setItem(
      'regelrecht-draft-notes:lawA',
      JSON.stringify([NOTE]),
    );
    const a = useDraftNotes(ref('lawA'));
    expect(a.drafts.value).toHaveLength(1);
    const b = useDraftNotes(ref('lawB'));
    expect(b.drafts.value).toHaveLength(0);
  });

  it('removeDraft drops the right index', () => {
    const { addDraft, removeDraft, drafts } = useDraftNotes(ref('w'));
    addDraft({ ...NOTE, body: { ...NOTE.body, value: 'a' } });
    addDraft({ ...NOTE, body: { ...NOTE.body, value: 'b' } });
    removeDraft(0);
    expect(drafts.value).toHaveLength(1);
    expect((drafts.value[0]!.body as TestBody).value).toBe('b');
  });

  it('clearDrafts empties storage', () => {
    const { addDraft, clearDrafts, drafts } = useDraftNotes(ref('w'));
    addDraft(NOTE);
    clearDrafts();
    expect(drafts.value).toHaveLength(0);
    expect(
      JSON.parse(localStorage.getItem('regelrecht-draft-notes:w') ?? 'null'),
    ).toEqual([]);
  });

  it('exports the raw sidecar notes + drafts (preserving cross-law notes)', async () => {
    // The committed file carries a note for ANOTHER law (federated sidecar).
    // The resolver would drop it; exportYaml must NOT - it re-parses the file.
    const otherLawNote = {
      ...NOTE,
      target: { ...NOTE.target, source: 'regelrecht://andere_wet' },
    };
    stubSidecar([{ ...NOTE, creator: 'Dienst Toeslagen' }, otherLawNote]);
    const { addDraft, exportYaml } = useDraftNotes(ref('w'));
    addDraft({ ...NOTE, __draft: true });
    const doc = loadExport(await exportYaml());
    expect(doc.$schema).toContain('annotation-schema.json');
    // 2 committed (incl. the other-law one) + 1 draft.
    expect(doc.annotations).toHaveLength(3);
    expect(doc.annotations[0]!.creator).toBe('Dienst Toeslagen');
    expect(doc.annotations[1]!.target!.source).toBe('regelrecht://andere_wet');
    // The internal __draft marker must never reach the exported YAML.
    expect(JSON.stringify(doc)).not.toContain('__draft');
  });

  it('exports drafts only when there is no sidecar yet (404)', async () => {
    stubSidecar(null);
    const { addDraft, exportYaml } = useDraftNotes(ref('w'));
    addDraft({ ...NOTE, __draft: true });
    const doc = loadExport(await exportYaml());
    expect(doc.annotations).toHaveLength(1);
  });

  it('does not fold long body strings in the export', async () => {
    stubSidecar(null);
    const longValue = 'x '.repeat(120).trim();
    const { addDraft, exportYaml } = useDraftNotes(ref('w'));
    addDraft({
      ...NOTE,
      body: { type: 'TextualBody', value: longValue, purpose: 'commenting' },
    });
    // lineWidth -1: the value stays on one logical line (no YAML fold marker).
    const reparsed = loadExport(await exportYaml());
    expect(reparsed.annotations[0]!.body.value).toBe(longValue);
  });
});

// saveToRepo now does ONE request: a PUT whose body is the JSON array of
// new drafts. No /data GET - the backend reads the base from the session
// branch and appends. Single stub for the PUT.
function stubSave(putResponse: unknown) {
  stubFetch(vi.fn(() => Promise.resolve(putResponse)));
}

describe('useDraftNotes.saveToRepo', () => {
  // Traject ref used across these saveToRepo tests - must look like
  // `{slug}-{8hex}` so the URL builder produces the canonical shape and
  // the assertions stay stable.
  const TRAJECT_REF = 'tarief-2026-3f4a8b2c';
  const TRAJECT_ROUTE = `/api/trajects/${TRAJECT_REF}`;

  beforeEach(() => {
    lastSavedPr.value = null;
  });

  it('PUTs only the new drafts as JSON, clears drafts, sets the PR', async () => {
    stubSave({
      ok: true,
      json: async () => ({
        pr: { url: 'https://github.com/x/y/pull/7', number: 7, branch: 'b' },
      }),
    });
    const { addDraft, saveToRepo, drafts } = useDraftNotes(
      ref('wet_op_de_zorgtoeslag'),
      ref(TRAJECT_REF),
    );
    addDraft({ ...NOTE, __draft: true });

    await saveToRepo();

    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
    const [url, opts] = fetchCalls()[0]!;
    expect(url).toBe(`${TRAJECT_ROUTE}/corpus/laws/wet_op_de_zorgtoeslag/annotations`);
    expect(opts.method).toBe('PUT');
    expect(opts.headers['Content-Type']).toBe('application/json');
    // No X-Editor-Session: the traject is explicit in the URL path, not
    // a session header.
    expect(opts.headers['X-Editor-Session']).toBeUndefined();
    const sent = JSON.parse(opts.body);
    expect(Array.isArray(sent)).toBe(true);
    expect(sent).toHaveLength(1);
    // The internal __draft marker must never reach the wire.
    expect(JSON.stringify(sent)).not.toContain('__draft');
    expect(drafts.value).toHaveLength(0);
    expect(lastSavedPr.value).toEqual({
      url: 'https://github.com/x/y/pull/7',
      number: 7,
      branch: 'b',
    });
  });

  it('uses the traject path; ignores any spurious source argument', async () => {
    stubSave({ ok: true, json: async () => ({ pr: null }) });
    const { addDraft, saveToRepo } = useDraftNotes(
      ref('wet_op_de_zorgtoeslag'),
      ref(TRAJECT_REF),
    );
    addDraft({ ...NOTE, __draft: true });

    // saveToRepo takes no source argument; passing one must not leak
    // into the URL.
    // The cast lets the test pass the argument the signature does not take.
    await (saveToRepo as (source: string) => Promise<unknown>)('amsterdam');

    expect(fetchCalls()[0]![0]).toBe(
      `${TRAJECT_ROUTE}/corpus/laws/wet_op_de_zorgtoeslag/annotations`,
    );
  });

  it('throws "no active traject" when called without a traject ref', async () => {
    // Per-tab traject lives in the URL; an editor session without one
    // has no business calling saveToRepo. The thrown shape matches the
    // requireTraject helper so the editor surfaces a clear error.
    stubSave({ ok: true, json: async () => ({ pr: null }) });
    const { addDraft, saveToRepo, drafts } = useDraftNotes(
      ref('wet_op_de_zorgtoeslag'),
      ref(null),
    );
    addDraft({ ...NOTE, __draft: true });

    await expect(saveToRepo()).rejects.toThrow(/active traject/);
    expect(globalThis.fetch).not.toHaveBeenCalled();
    expect(drafts.value).toHaveLength(1);
  });

  it('refuses to save when no law is open, without a request', async () => {
    stubSave({ ok: true, json: async () => ({ pr: null }) });
    const { saveToRepo, publishNote } = useDraftNotes(ref(null), ref(TRAJECT_REF));

    await expect(saveToRepo()).rejects.toThrow(/open law/);
    await expect(publishNote({ ...NOTE, __draft: true })).rejects.toThrow(/open law/);
    expect(globalThis.fetch).not.toHaveBeenCalled();
  });

  it('keeps the existing PR badge on a NoChange re-save', async () => {
    // Regression for hostile-review #4: a PR-less 200 (every note already
    // committed) must NOT null an existing badge and must report noChange
    // so the UI can say "already saved" instead of looking like loss.
    // `branch` is not part of SavedPr; assigned via a variable so the extra
    // field the backend sends stays in the fixture.
    const earlierPr = { url: 'https://github.com/x/y/pull/3', number: 3, branch: 'b' };
    lastSavedPr.value = earlierPr;
    stubSave({ ok: true, json: async () => ({ pr: null, no_change: true }) });
    const { addDraft, saveToRepo, drafts } = useDraftNotes(
      ref('w'),
      ref(TRAJECT_REF),
    );
    addDraft({ ...NOTE, __draft: true });

    const result = await saveToRepo();

    expect(result).toEqual({ pr: null, noChange: true });
    // Badge untouched - the earlier PR is still shown.
    expect(lastSavedPr.value).toEqual({
      url: 'https://github.com/x/y/pull/3',
      number: 3,
      branch: 'b',
    });
    // Drafts cleared (they are already upstream) - that is fine because
    // the caller now has noChange to show an explicit message.
    expect(drafts.value).toHaveLength(0);
  });

  it('throws the editor-api message and keeps drafts on a 400', async () => {
    stubSave({
      ok: false,
      status: 400,
      headers: { get: () => 'text/plain; charset=utf-8' },
      text: async () => 'Notes are not valid against the annotation schema',
    });
    const { addDraft, saveToRepo, drafts } = useDraftNotes(
      ref('w'),
      ref(TRAJECT_REF),
    );
    addDraft({ ...NOTE, __draft: true });

    await expect(saveToRepo()).rejects.toThrow(/not valid against/);
    expect(drafts.value).toHaveLength(1);
  });

  it('clears the saved law, not the law switched to mid-save', async () => {
    // Regression for the hostile-review finding: a law switch during the
    // PUT must not wipe the new law's drafts nor leave the saved law's.
    let resolvePut: ((value: unknown) => void) | undefined;
    stubFetch(vi.fn(
      () => new Promise((r) => { resolvePut = r; }),
    ));
    const lawId = ref('lawA');
    const { addDraft, saveToRepo, drafts } = useDraftNotes(lawId, ref(TRAJECT_REF));
    addDraft({ ...NOTE, __draft: true }); // a draft on lawA

    const p = saveToRepo(); // snapshots id = 'lawA'
    // User switches to lawB before the PUT resolves; lawB has its own draft.
    localStorage.setItem(
      'regelrecht-draft-notes:lawB',
      JSON.stringify([{ ...NOTE, body: { ...NOTE.body, value: 'B' } }]),
    );
    lawId.value = 'lawB';
    await Promise.resolve(); // let watch(lawId) resync drafts to lawB
    expect(resolvePut).toBeDefined();
    resolvePut!({ ok: true, json: async () => ({ pr: null }) });
    await p;

    // lawB's draft (on screen now) is untouched.
    expect(drafts.value).toHaveLength(1);
    expect((drafts.value[0]!.body as TestBody).value).toBe('B');
    // lawA's drafts were cleared in storage (they are in the PR).
    expect(
      JSON.parse(localStorage.getItem('regelrecht-draft-notes:lawA') ?? 'null'),
    ).toEqual([]);
  });
});
