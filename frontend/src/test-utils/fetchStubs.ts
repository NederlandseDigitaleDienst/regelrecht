// Fetch stubs for the composable tests.
//
// `stubFetch` puts a spy on the global fetch through `vi.stubGlobal`, and
// importing this module registers an `afterEach` that undoes it, so a stub
// never leaks into the next test. (A plain `globalThis.fetch = ...` survives
// `vi.restoreAllMocks`.)
import { afterEach, vi, type Mock } from 'vitest';

afterEach(() => {
  vi.unstubAllGlobals();
});

/** Any function `vi.fn` accepts (not a class); vitest does not export that type itself. */
type Impl = Extract<NonNullable<Parameters<typeof vi.fn>[0]>, (...args: never[]) => unknown>;

/**
 * Install `impl` as the global fetch and return the spy. A plain function is
 * wrapped in `vi.fn`; a spy is installed as is.
 */
export function stubFetch<T extends Impl>(impl: T | Mock<T>): Mock<T>;
export function stubFetch(): Mock<() => Promise<Response>>;
export function stubFetch(impl: Impl = () => Promise.resolve(res())): Mock {
  const spy = vi.isMockFunction(impl) ? (impl as Mock) : vi.fn(impl);
  vi.stubGlobal('fetch', spy);
  return spy;
}

export interface ResOptions {
  ok?: boolean;
  status?: number;
  body?: unknown;
  etag?: string | null;
  json?: unknown;
}

/**
 * A Response-like object: `body` is what `text()` returns (an object is
 * serialised), `json` is what `json()` returns (falling back to `body`), and
 * `etag` lands in the headers. It carries only the members the composables
 * read, hence the cast.
 */
export function res({ ok = true, status = 200, body = '', etag = null, json = null }: ResOptions = {}): Response {
  const headers = new Map<string, string>();
  if (etag) headers.set('etag', etag);
  if (json) headers.set('content-type', 'application/json');
  else if (typeof body === 'string') headers.set('content-type', 'text/plain');
  return {
    ok,
    status,
    headers: {
      get: (k: string) => headers.get(k.toLowerCase()) ?? null,
    },
    async text() {
      return typeof body === 'string' ? body : JSON.stringify(body);
    },
    async json() {
      return json ?? body;
    },
  } as unknown as Response;
}

/** One page of a paginated list endpoint, as `res` sees it. */
export function pageRes({ data = [], total = data.length }: { data?: unknown[]; total?: number } = {}): Response {
  return res({ json: { data, total, limit: 50, offset: 0 } });
}

/** A real `Response` with a JSON body. */
export function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}
