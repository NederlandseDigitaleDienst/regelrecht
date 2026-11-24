// Calls to the server, as every API client of this app makes them: JSON in
// and out, through the session of sessie.ts, and the server's own reason when
// it refuses (`{ "fout": "<why>" }`).
import { ApiError } from '@regelrecht/frontend-shared';
import { reden, sessie } from './sessie';

export { reden } from './sessie';

/**
 * The message of an error thrown by a call, for a screen to show: the
 * server's own reason, or a short line in its place (`reden`), never the raw
 * body of an answer. Also for a call made without the session, such as the
 * login's.
 */
export function foutTekst(e: unknown): string {
  // An error the app's core made itself carries its message and no body.
  if (e instanceof ApiError) return e.body ? reden(e.body, e.status) : e.message || reden('', e.status);
  if (e instanceof Error) return e.message;
  return String(e);
}

/** Whether `e` says the session holds no login (401). */
export const nietIngelogd = (e: unknown): boolean => e instanceof ApiError && e.status === 401;

/** `GET path`, as JSON. */
export function haal<T>(path: string): Promise<T> {
  return sessie().haal<T>(path);
}

/**
 * `method path` with the JSON body `body`, as JSON. `sleutel`, when given, is
 * the idempotency key: the server answers a request sent again under the same
 * key as the first time and records nothing twice.
 */
export function stuur<T>(method: string, path: string, body: unknown, sleutel?: string): Promise<T> {
  return sessie().stuur<T>(method, path, body, sleutel);
}

/** A new idempotency key, for one request a screen may send twice. */
export const nieuweSleutel = (): string => crypto.randomUUID();
