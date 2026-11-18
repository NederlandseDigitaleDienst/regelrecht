import { describe, it, expect, vi, afterEach } from 'vitest';
import { apiFetch, apiFetchJson, apiFetchText, ApiError } from './apiFetch';

function mockResponse({
  status = 200,
  body = '',
  contentType = null,
  json = undefined,
}: { status?: number; body?: string; contentType?: string | null; json?: unknown } = {}) {
  const headers = new Headers();
  if (contentType) headers.set('content-type', contentType);
  return {
    ok: status >= 200 && status < 300,
    status,
    headers,
    text: async () => body,
    json: async () => (json !== undefined ? json : JSON.parse(body)),
  };
}

const realFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = realFetch;
});

describe('apiFetch', () => {
  it('resolves with the raw Response on ok so headers stay readable', async () => {
    const res = mockResponse({ status: 200, body: 'x' });
    res.headers.set('ETag', '"abc"');
    globalThis.fetch = vi.fn().mockResolvedValue(res);

    const out = await apiFetch('/api/thing');
    expect(out).toBe(res);
    expect(out.headers.get('ETag')).toBe('"abc"');
  });

  it('strips wrapper options before calling fetch', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(mockResponse());
    await apiFetch('/api/thing', {
      method: 'PUT',
      allowStatuses: [404],
      errorMessage: 'nope',
    });
    const [, init] = vi.mocked(globalThis.fetch).mock.calls[0]!;
    expect(init).toEqual({ method: 'PUT' });
  });

  it('throws ApiError with status, body and contentType on non-ok', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      mockResponse({ status: 403, body: 'geen schrijfrechten', contentType: 'text/plain; charset=utf-8' }),
    );
    const err = await apiFetch('/api/thing').catch((e) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err.status).toBe(403);
    expect(err.body).toBe('geen schrijfrechten');
    expect(err.contentType).toBe('text/plain; charset=utf-8');
    expect(err.message).toBe('geen schrijfrechten');
  });

  it('falls back to "HTTP <status>" when the body is empty', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(mockResponse({ status: 500 }));
    const err = await apiFetch('/api/thing').catch((e) => e);
    expect(err.message).toBe('HTTP 500');
    expect(err.body).toBe('');
  });

  it('falls back to "HTTP <status>" when the body read throws', async () => {
    const res = mockResponse({ status: 502 });
    res.text = async () => {
      throw new Error('stream gone');
    };
    globalThis.fetch = vi.fn().mockResolvedValue(res);
    const err = await apiFetch('/api/thing').catch((e) => e);
    expect(err.message).toBe('HTTP 502');
  });

  it('resolves allowStatuses statuses instead of throwing', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(mockResponse({ status: 404 }));
    const res = await apiFetch('/api/thing', { allowStatuses: [404] });
    expect(res.status).toBe(404);
  });

  it('supports a static errorMessage', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(mockResponse({ status: 500, body: 'boom' }));
    const err = await apiFetch('/api/thing', { errorMessage: 'Kon niet laden' }).catch((e) => e);
    expect(err.message).toBe('Kon niet laden');
    expect(err.body).toBe('boom');
  });

  it('supports an errorMessage function receiving status, body and contentType', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(
      mockResponse({ status: 503, body: 'b', contentType: 'text/plain' }),
    );
    const err = await apiFetch('/api/thing', {
      errorMessage: (status, body, contentType) => `mislukt: ${status} (${body}, ${contentType})`,
    }).catch((e) => e);
    expect(err.message).toBe('mislukt: 503 (b, text/plain)');
  });

  it('lets network errors propagate unchanged', async () => {
    const netErr = new TypeError('Failed to fetch');
    globalThis.fetch = vi.fn().mockRejectedValue(netErr);
    await expect(apiFetch('/api/thing')).rejects.toBe(netErr);
  });
});

describe('apiFetchJson / apiFetchText', () => {
  it('apiFetchJson parses the body', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(mockResponse({ json: [{ a: 1 }] }));
    expect(await apiFetchJson('/api/list')).toEqual([{ a: 1 }]);
  });

  it('apiFetchText returns the body text', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(mockResponse({ body: 'yaml: 1' }));
    expect(await apiFetchText('/api/law')).toBe('yaml: 1');
  });

  it('both throw ApiError on non-ok', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(mockResponse({ status: 404 }));
    await expect(apiFetchJson('/api/list')).rejects.toBeInstanceOf(ApiError);
    globalThis.fetch = vi.fn().mockResolvedValue(mockResponse({ status: 404 }));
    await expect(apiFetchText('/api/law')).rejects.toBeInstanceOf(ApiError);
  });
});
