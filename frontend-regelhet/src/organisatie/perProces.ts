// Asking every process of the organisation at once, through the front. The
// front passes a short list of a process's GET routes on with a statement of
// who is logged in (`GET /api/processen/{id}/api/{pad}`, packages/regelhet/
// src/organisatie/relay.rs); a screen over all processes asks each and keeps
// what each answered, or why it did not, so one process that fails leaves the
// others on the screen.
import { foutTekst, haal } from '../gedeeld/http';
import type { Proces } from './api';

/** What one process answered, or why it did not. */
export type ProcesAntwoord<T> = { proces: Proces; antwoord: T } | { proces: Proces; fout: string };

/** The front's address of the route `pad` (after `/api/`, such as `backoffice/werkvoorraad`) of the process `id`. */
export function procesPad(id: string, pad: string): string {
  return `/api/processen/${encodeURIComponent(id)}/api/${pad}`;
}

/** `GET pad` (after `/api/`) of the process `id`, through the front. */
export function haalBijProces<T>(id: string, pad: string): Promise<T> {
  return haal<T>(procesPad(id, pad));
}

/**
 * `vraag` for every process of `processen` at once: per process, in the order
 * of `processen`, its answer or the reason it gave none. One process that
 * fails or does not answer never takes the others with it.
 */
export async function perProces<T>(processen: readonly Proces[], vraag: (proces: Proces) => Promise<T>): Promise<ProcesAntwoord<T>[]> {
  const uitkomsten = await Promise.allSettled(processen.map((p) => vraag(p)));
  return uitkomsten.map((u, i) => {
    const proces = processen[i] as Proces; // allSettled keeps one outcome per process, in order
    return u.status === 'fulfilled' ? { proces, antwoord: u.value } : { proces, fout: foutTekst(u.reason) };
  });
}

/** Whether `a` is an answer, not a failure. */
export const beantwoord = <T>(a: ProcesAntwoord<T>): a is { proces: Proces; antwoord: T } => 'antwoord' in a;
