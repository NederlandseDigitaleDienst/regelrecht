// @vitest-environment node
// The app's session against a stub of the core: which command each call of
// the citizen's screens becomes, and what a screen gets for each error.
import { ApiError } from '@regelrecht/frontend-shared';
import { describe, expect, it, vi } from 'vitest';
import { alsFout, ikUit, kernSessie, melding } from './kern';
import type { AppStatus, Invoke, KernFout } from './kern';

const status: AppStatus = {
  server: 'http://wetsproces.example',
  config_problem: null,
  holder: { bsn: '999990019', name: 'Test Burger' },
  credential_expires_at: 2_000_000_000,
  credential_expired: false,
  key_storage: { kind: 'memory' },
  hardware_backed: false,
  test_bsns: ['999990019'],
};

/** A stub of `invoke` that records each command and answers `antwoord`. */
function stub(antwoord: (commando: string) => unknown) {
  const aanroepen: [string, Record<string, unknown> | undefined][] = [];
  const invoke = (async (commando: string, args?: Record<string, unknown>) => {
    aanroepen.push([commando, args]);
    const a = antwoord(commando);
    if (a instanceof Error || (a && typeof a === 'object' && 'kind' in a)) throw a;
    return a;
  }) as Invoke;
  return { invoke, aanroepen };
}

describe('the session of the app', () => {
  it('sends each call of the citizen to its command', async () => {
    const { invoke, aanroepen } = stub(() => ({}));
    const s = kernSessie(invoke);
    await s.haal('/api/casus');
    await s.haal('/api/formulier');
    await s.haal('/api/dossier');
    await s.haal('/api/dossier/waarom/b%231');
    expect(aanroepen).toEqual([
      ['casus', undefined],
      ['formulier', undefined],
      ['dossier', undefined],
      ['waarom', { besluit: 'b#1' }],
    ]);
  });

  it('sends a submission with its idempotency key, and refuses one without', async () => {
    const { invoke, aanroepen } = stub(() => ({ id: 'a#1' }));
    const s = kernSessie(invoke);
    const indiening = { velden: { naam: 'T. Burger' } };
    await expect(s.stuur('POST', '/api/indieningen', indiening, 'sleutel-1')).resolves.toEqual({ id: 'a#1' });
    expect(aanroepen).toEqual([['indienen', { indiening, sleutel: 'sleutel-1' }]]);
    await expect(s.stuur('POST', '/api/indieningen', indiening)).rejects.toThrow(/idempotentiesleutel/);
  });

  it('offers no route outside the citizen side', async () => {
    const { invoke, aanroepen } = stub(() => ({}));
    const s = kernSessie(invoke);
    await expect(s.haal('/api/backoffice/werkvoorraad')).rejects.toThrow(/niet aan/);
    await expect(s.haal('/api/klok')).rejects.toThrow(/niet aan/);
    await expect(s.stuur('PUT', '/api/klok', {})).rejects.toThrow(/niet aan/);
    expect(aanroepen).toEqual([]);
  });

  it('answers who is logged in from the status of the core', async () => {
    const { invoke } = stub(() => status);
    await expect(kernSessie(invoke).haal('/api/ik')).resolves.toMatchObject({
      burger: true,
      identiteit: '999990019',
      naam: 'Test Burger',
      cellen: [],
    });
    expect(ikUit({ ...status, holder: null })).toBeNull();
  });

  it('gives the service’s refusal to the screen as the browser would get it', async () => {
    const { invoke } = stub(() => ({ kind: 'service', status: 422, body: { fout: 'het veld ontbreekt' } }));
    const fout = await kernSessie(invoke)
      .haal('/api/formulier')
      .catch((e: unknown) => e);
    expect(fout).toBeInstanceOf(ApiError);
    expect((fout as ApiError).status).toBe(422);
    expect((fout as ApiError).message).toBe('het veld ontbreekt');
  });

  it('sends the citizen back to activation when the credential lapses or is refused', async () => {
    for (const kind of ['credential_expired', 'credential_refused', 'not_activated'] as const) {
      const bij = vi.fn();
      const { invoke } = stub(() => ({ kind }));
      const fout = await kernSessie(invoke, bij)
        .haal('/api/dossier')
        .catch((e: unknown) => e);
      expect(bij).toHaveBeenCalledWith({ kind });
      expect((fout as ApiError).status).toBe(401);
    }
  });

  it('names a network error plainly, without sending the citizen back', async () => {
    const bij = vi.fn();
    const { invoke } = stub(() => ({ kind: 'network', detail: 'connection refused' }));
    const fout = await kernSessie(invoke, bij)
      .haal('/api/casus')
      .catch((e: unknown) => e);
    expect(bij).not.toHaveBeenCalled();
    expect(fout).not.toBeInstanceOf(ApiError);
    expect((fout as Error).message).toMatch(/niet bereikbaar/);
  });

  it('shows no page of the platform the core got back in place of an answer', async () => {
    const pagina = '<html><body><h1>503 Service Unavailable</h1></body></html>';
    const { invoke } = stub(() => ({ kind: 'service', status: 503, body: pagina }));
    const fout = await kernSessie(invoke, vi.fn())
      .haal('/api/casus')
      .catch((e: unknown) => e);
    expect((fout as ApiError).message).toMatch(/niet bereikbaar/);
    expect((fout as ApiError).message).not.toContain('<');
  });

  it('has a text for every error of the core', () => {
    const fouten: KernFout[] = [
      { kind: 'not_configured', detail: 'x' },
      { kind: 'not_activated' },
      { kind: 'credential_expired' },
      { kind: 'credential_refused' },
      { kind: 'no_demo', detail: 'x' },
      { kind: 'network', detail: 'x' },
      { kind: 'service', status: 500, body: { fout: 'kapot' } },
      { kind: 'protocol', detail: 'x' },
      { kind: 'key_store', detail: 'x' },
      { kind: 'input', detail: 'x' },
    ];
    for (const f of fouten) expect(melding(f).length, f.kind).toBeGreaterThan(0);
    expect(alsFout(new Error('los')).message).toBe('los');
  });
});
