import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref, nextTick } from 'vue';
import { useTrajectDocuments } from './useTrajectDocuments';
import { res, stubFetch } from '../test-utils/fetchStubs';

// A fetch call as the mocks below record it.
interface FetchCall {
  url: string;
  opts: RequestInit;
}

beforeEach(() => {
  localStorage.clear();
  vi.restoreAllMocks();
});

describe('useTrajectDocuments', () => {
  it('fetches the documents list for the active traject', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(
      res({ json: { documents: [{ path: 'notes.md' }, { path: 'mvt/concept.md' }] } }),
    );
    stubFetch(fetchSpy);

    const trajectRef = ref('mig-1a2b3c4d');
    const { documents, fetchList } = useTrajectDocuments(trajectRef);
    await fetchList();

    expect(fetchSpy).toHaveBeenCalledWith('/api/trajects/mig-1a2b3c4d/corpus/documents', {});
    expect(documents.value.map((d) => d.path)).toEqual([
      'notes.md',
      'mvt/concept.md',
    ]);
  });

  it('uploads a document as multipart and refreshes the list', async () => {
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'POST') {
        return res({ status: 202, json: { target_path: 'rapport.md' } });
      }
      return res({ json: { documents: [] } });
    });

    const trajectRef = ref('mig-1a2b3c4d');
    const { uploadDocument } = useTrajectDocuments(trajectRef);
    const file = new File([new Uint8Array([1, 2, 3])], 'Rapport.pdf', {
      type: 'application/pdf',
    });
    const result = await uploadDocument(file);

    expect(result).toEqual({ ok: true, targetPath: 'rapport.md' });
    const post = calls.find((c) => c.opts.method === 'POST');
    expect(post).toBeTruthy();
    expect(post?.url).toBe('/api/trajects/mig-1a2b3c4d/corpus/documents/upload');
    expect(post?.opts.body).toBeInstanceOf(FormData);
    expect((post?.opts.body as FormData).get('file')).toBeInstanceOf(File);
    // Content-Type must NOT be set - the browser adds the multipart boundary.
    expect(post?.opts.headers).toBeUndefined();
    // A successful upload refreshes the document list (GET after the POST).
    expect(
      calls.some((c) => c.url.endsWith('/corpus/documents') && c.opts.method !== 'POST'),
    ).toBe(true);
  });

  it('handles a markdown pass-through upload (201) the same as a conversion (202)', async () => {
    // A `.md` upload is stored as-is server-side and answers 201 Created (the
    // resource already exists) instead of 202 Accepted (async conversion). The
    // composable must treat any ok status the same: read `target_path`, refresh.
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (opts?.method === 'POST') {
        return res({ status: 201, json: { target_path: 'notities.md' } });
      }
      return res({ json: { documents: [{ path: 'notities.md' }] } });
    });

    const trajectRef = ref('mig-1a2b3c4d');
    const { uploadDocument } = useTrajectDocuments(trajectRef);
    const file = new File([new TextEncoder().encode('# Titel\n')], 'Notities.md', {
      type: 'text/markdown',
    });
    const result = await uploadDocument(file);

    expect(result).toEqual({ ok: true, targetPath: 'notities.md' });
    expect(
      calls.some((c) => c.url.endsWith('/corpus/documents') && c.opts.method !== 'POST'),
    ).toBe(true);
  });

  it('captures the ETag on open and sends it back as If-Match on save', async () => {
    const trajectRef = ref('mig-1a2b3c4d');
    // 1st call: openDocument GET.
    // 2nd call: list refresh on trajectRef change (the immediate watch).
    // 3rd call: saveCurrent PUT.
    const calls: FetchCall[] = [];
    stubFetch(async (url, opts) => {
      calls.push({ url, opts: opts ?? {} });
      if (url === '/api/trajects/mig-1a2b3c4d/corpus/documents') {
        return res({ json: { documents: [{ path: 'notes.md' }] } });
      }
      if (opts?.method === 'PUT') {
        return res({ status: 200, etag: '"new"', json: { etag: '"new"' } });
      }
      return res({ body: '# Existing', etag: '"v1"' });
    });

    const docs = useTrajectDocuments(trajectRef);
    await nextTick(); // let the immediate watch fire
    await docs.openDocument('notes.md');
    expect(docs.currentEtag.value).toBe('"v1"');

    docs.currentBody.value = '# Updated';
    await docs.saveCurrent();

    const put = calls.find((c) => c.opts?.method === 'PUT');
    expect(put).toBeTruthy();
    // saveCurrent sends a plain header object.
    expect((put?.opts.headers as Record<string, string>)['If-Match']).toBe('"v1"');
    expect(docs.currentEtag.value).toBe('"new"');
  });

  it('surfaces a 412 as a conflict instead of overwriting silently', async () => {
    const trajectRef = ref('mig-1a2b3c4d');
    stubFetch(async (url, opts) => {
      if (url.endsWith('/documents')) return res({ json: { documents: [] } });
      if (opts?.method === 'PUT') return res({ ok: false, status: 412 });
      return res({ body: '# Existing', etag: '"v1"' });
    });

    const docs = useTrajectDocuments(trajectRef);
    await docs.openDocument('notes.md');
    docs.currentBody.value = '# Edited';
    const result = await docs.saveCurrent();

    expect(result).toEqual({ ok: false, conflict: true });
    expect(docs.conflict.value).toMatch(/gewijzigd/i);
  });

  it('refuses to operate without a trajectRef', async () => {
    const trajectRef = ref<string | null>(null);
    const docs = useTrajectDocuments(trajectRef);
    docs.currentPath.value = 'notes.md';
    docs.currentBody.value = 'x';
    await expect(docs.saveCurrent()).rejects.toThrow(/traject/);
  });

  it('keeps the `/` literal so the backend wildcard receives the hierarchy', async () => {
    // URL-builder unit test: paths the backend accepts (lowercase
    // `[a-z0-9._-]` segments) must reach the API with their slashes
    // preserved, not percent-encoded. End-to-end rejection of weird
    // characters is the backend's job (see `validate_document_path`).
    const trajectRef = ref('mig-1a2b3c4d');
    const fetchSpy = vi.fn().mockResolvedValue(
      res({ body: '# Concept', etag: '"v1"' }),
    );
    stubFetch(fetchSpy);

    const docs = useTrajectDocuments(trajectRef);
    await docs.openDocument('mvt/concept-v2.md');

    const url = fetchSpy.mock.calls.find((c) =>
      c[0].includes('mvt'),
    )?.[0];
    expect(url).toBe(
      '/api/trajects/mig-1a2b3c4d/corpus/documents/mvt/concept-v2.md',
    );
  });

  it('dropDraft discards local edits by reverting the body to the saved baseline', async () => {
    const trajectRef = ref('mig-1a2b3c4d');
    stubFetch(async (url) => {
      if (url.endsWith('/documents')) return res({ json: { documents: [] } });
      return res({ body: '# Server', etag: '"v1"' });
    });

    const docs = useTrajectDocuments(trajectRef);
    await docs.openDocument('notes.md');
    expect(docs.currentBody.value).toBe('# Server');

    // Simulate an edit that diverges from the saved (server) baseline.
    docs.currentBody.value = '# Local edit';
    docs.dropDraft();

    // The edit is reverted, not just the localStorage draft cleared — so the
    // document is clean again and won't re-trip the leave-guard on reopen
    // ("Negeer wijzigingen en sluit" truly discards).
    expect(docs.currentBody.value).toBe('# Server');
    expect(docs.savedBody.value).toBe('# Server');
    expect(docs.docError.value).toBeNull();
  });

  it('dropDraft clears an orphan draft for a document that does not exist on the server', async () => {
    // Mirrors a rejected document-review proposal: the target document is
    // never pushed (404), a proposal/edit gets typed into it and debounced
    // to localStorage, and "Verwerpen" must not leave that draft behind -
    // reopening the (still-nonexistent) document later would otherwise
    // resurrect it as a 'draft-present' notice forever.
    vi.useFakeTimers();
    try {
      const trajectRef = ref('mig-1a2b3c4d');
      stubFetch(async (url) => {
        if (url.endsWith('/documents')) return res({ json: { documents: [] } });
        return res({ ok: false, status: 404 });
      });

      const docs = useTrajectDocuments(trajectRef);
      await docs.openDocument('proposal.md');
      expect(docs.docError.value?.kind).toBe('not-found');

      docs.currentBody.value = '# Voorstel';
      await vi.advanceTimersByTimeAsync(600);
      expect(
        localStorage.getItem('regelrecht-doc-draft:mig-1a2b3c4d:proposal.md'),
      ).toBe('# Voorstel');

      docs.dropDraft();

      expect(
        localStorage.getItem('regelrecht-doc-draft:mig-1a2b3c4d:proposal.md'),
      ).toBeNull();
      expect(docs.currentBody.value).toBe('');
      expect(docs.docError.value).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});
