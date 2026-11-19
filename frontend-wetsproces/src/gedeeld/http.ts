// Calls to the server, as every API client of this app makes them: JSON in
// and out, the session from sessie.ts, and the server's own reason when it
// refuses (`{ "fout": "<why>" }`).
import { ApiError, apiFetchJson } from '@regelrecht/frontend-shared';
import { metSessie } from './sessie';

/** The reason in an error answer of the server, or its plain text. */
export function reden(body: string): string {
  try {
    const parsed: unknown = JSON.parse(body);
    if (parsed && typeof parsed === 'object' && 'fout' in parsed && typeof parsed.fout === 'string') {
      return parsed.fout;
    }
  } catch {
    // Not JSON: axum's own rejection of a body that does not parse.
  }
  return body;
}

/** The message of an error thrown by a call, for a screen to show. */
export function foutTekst(e: unknown): string {
  if (e instanceof ApiError) return e.message || `De server antwoordde met ${e.status}.`;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** Whether `e` says the session holds no login (401). */
export const nietIngelogd = (e: unknown): boolean => e instanceof ApiError && e.status === 401;

const errorMessage = (_status: number, body: string) => reden(body);

/** `GET path`, as JSON. */
export function haal<T>(path: string): Promise<T> {
  return apiFetchJson<T>(path, { ...metSessie(), errorMessage });
}

/**
 * `method path` with the JSON body `body`, as JSON. `sleutel`, when given, is
 * the idempotency key: the server answers a request sent again under the same
 * key as the first time and records nothing twice.
 */
export function stuur<T>(method: string, path: string, body: unknown, sleutel?: string): Promise<T> {
  const headers: Record<string, string> = { 'content-type': 'application/json' };
  if (sleutel) headers['idempotency-key'] = sleutel;
  return apiFetchJson<T>(path, {
    ...metSessie({ method, headers, body: JSON.stringify(body) }),
    errorMessage,
  });
}

/** A new idempotency key, for one request a screen may send twice. */
export const nieuweSleutel = (): string => crypto.randomUUID();
