// @vitest-environment node
// What a screen shows when a call fails: the server's reason, or a short line
// in its place, never the body of a page the platform put in front of it.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '@regelrecht/frontend-shared';
import { foutTekst } from './http';
import { NIET_BEREIKBAAR, cookieSessie, reden } from './sessie';

const PAGINA = '<!DOCTYPE html><html><head><title>503 Service Unavailable</title></head><body><h1>Service Unavailable</h1></body></html>';

afterEach(() => vi.unstubAllGlobals());

describe('the message of a failed call', () => {
  it("is the server's own reason when it gives one", () => {
    expect(reden(JSON.stringify({ fout: 'Een BSN bestaat uit negen cijfers.' }), 400)).toBe('Een BSN bestaat uit negen cijfers.');
  });

  it('is a short line for a page of the platform, never its body', () => {
    for (const status of [502, 503, 504, undefined]) {
      expect(reden(PAGINA, status)).toBe(NIET_BEREIKBAAR);
    }
    expect(NIET_BEREIKBAAR).toContain('blijft staan');
  });

  it('names the status of a refusal without a reason, not its text', () => {
    const tekst = reden('Failed to deserialize the JSON body into the target type', 422);
    expect(tekst).toContain('422');
    expect(tekst).not.toContain('deserialize');
  });

  it('comes from the cookie session for every kind of failure', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(PAGINA, { status: 503, headers: { 'content-type': 'text/html' } })));
    const fout = await cookieSessie.stuur('POST', '/api/indieningen', {}).catch((e: unknown) => e);
    expect(foutTekst(fout)).toBe(NIET_BEREIKBAAR);

    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('Failed to fetch'))));
    const weg = await cookieSessie.haal('/api/dossier').catch((e: unknown) => e);
    expect(foutTekst(weg)).toBe(NIET_BEREIKBAAR);
  });

  it('reads a 200 that is a page of the platform as unreachable, not as its parse error', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(PAGINA, { status: 200, headers: { 'content-type': 'text/html' } })));
    const fout = await cookieSessie.haal('/api/dossier').catch((e: unknown) => e);
    expect(foutTekst(fout)).toBe(NIET_BEREIKBAAR);
  });

  it('leaves an error of the code itself as it is', () => {
    expect(foutTekst(new TypeError("Cannot read properties of undefined (reading 'x')"))).toContain('undefined');
  });

  it('is never the raw body, also for an error made outside the session', () => {
    expect(foutTekst(new ApiError(PAGINA, { status: 503, body: PAGINA }))).toBe(NIET_BEREIKBAAR);
    // An error the app's core made has a message and no body.
    expect(foutTekst(new ApiError('Deze app is nog niet geactiveerd.', { status: 401 }))).toBe('Deze app is nog niet geactiveerd.');
  });
});
