import { beforeEach, describe, expect, it, vi } from 'vitest';
import storeSource from '../store/demoStore.ts?raw';
import { explainWhy, lockWhy, toBlocks, PASSWORD_KEY, probeWhy, unlockWhy, whyAvailable, whyUnlocked, WhyError } from './why';

const json = (status: number, body: unknown) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

beforeEach(() => {
  window.localStorage.clear();
  lockWhy();
  whyAvailable.value = false;
});

describe('probeWhy', () => {
  it('sees a server that says it is available', async () => {
    expect(await probeWhy(async () => json(200, { available: true }))).toBe(true);
  });

  it('does not mistake index.html for a server', async () => {
    const html = async () => new Response('<!doctype html>', { status: 200, headers: { 'content-type': 'text/html' } });
    expect(await probeWhy(html)).toBe(false);
  });

  it('treats a failing proxy or network as no server', async () => {
    expect(await probeWhy(async () => new Response('', { status: 502 }))).toBe(false);
    expect(await probeWhy(async () => { throw new TypeError('offline'); })).toBe(false);
  });
});

describe('unlocking', () => {
  it('stores a password the server accepts, under its own key', async () => {
    whyAvailable.value = true;
    expect(await unlockWhy('geheim', async () => new Response(null, { status: 204 }))).toBe('ok');
    expect(whyUnlocked.value).toBe(true);
    expect(window.localStorage.getItem(PASSWORD_KEY)).toBe('geheim');
  });

  it('calls a password a header cannot carry wrong, not unavailable', async () => {
    const fetchImpl = vi.fn();
    expect(await unlockWhy('geheim🔑', fetchImpl)).toBe('wrong');
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it('stores nothing for a wrong one', async () => {
    whyAvailable.value = true;
    expect(await unlockWhy('fout', async () => json(401, {}))).toBe('wrong');
    expect(whyUnlocked.value).toBe(false);
    expect(window.localStorage.getItem(PASSWORD_KEY)).toBeNull();
  });

  it('stays locked while no server is there, whatever is stored', async () => {
    await unlockWhy('geheim', async () => new Response(null, { status: 204 }));
    whyAvailable.value = false;
    expect(whyUnlocked.value).toBe(false);
  });

  it('is not the demo state, so resetting the demo leaves it alone', () => {
    // demoStore persists under its own key and resets by replacing that
    // object; this pins that the two keys stay apart.
    const store = storeSource;
    const stateKey = store.match(/const STORAGE_KEY = '([^']+)'/)?.[1];
    expect(stateKey).toBeTruthy();
    expect(stateKey).not.toBe(PASSWORD_KEY);
    expect(store).not.toMatch(/localStorage\??\.(clear|removeItem)/);
  });
});

describe('explainWhy', () => {
  it('sends the password and hands over the text as it streams', async () => {
    whyAvailable.value = true;
    await unlockWhy('geheim', async () => new Response(null, { status: 204 }));
    const fetchImpl = vi.fn(
      async (_url: RequestInfo | URL, _init?: RequestInit) => new Response('Je hebt recht.', { status: 200, headers: { 'content-type': 'text/plain' } }),
    );
    const seen: string[] = [];
    const text = await explainWhy({ law: { name: 'x' } }, { fetchImpl, onText: (t) => seen.push(t) });
    expect(text).toBe('Je hebt recht.');
    expect(seen.at(-1)).toBe('Je hebt recht.');
    // explainWhy passes its headers as a plain object.
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Record<string, string> | undefined;
    expect(headers?.['x-demo-password']).toBe('geheim');
  });

  it('forgets a password the server no longer accepts', async () => {
    whyAvailable.value = true;
    await unlockWhy('oud', async () => new Response(null, { status: 204 }));
    await expect(explainWhy({}, { fetchImpl: async () => json(401, {}) })).rejects.toMatchObject({ kind: 'locked' });
    expect(whyUnlocked.value).toBe(false);
    expect(window.localStorage.getItem(PASSWORD_KEY)).toBeNull();
  });

  it('tells a busy server from a failing one', async () => {
    await expect(explainWhy({}, { fetchImpl: async () => json(429, {}) })).rejects.toBeInstanceOf(WhyError);
    await expect(explainWhy({}, { fetchImpl: async () => json(429, {}) })).rejects.toMatchObject({ kind: 'busy' });
    await expect(explainWhy({}, { fetchImpl: async () => json(502, {}) })).rejects.toMatchObject({ kind: 'failed' });
  });
});

describe('toBlocks', () => {
  it('splits paragraphs on a blank line and joins wrapped lines', () => {
    expect(toBlocks('Een\nzin.\n\nTwee.')).toEqual([{ text: 'Een zin.' }, { text: 'Twee.' }]);
  });

  it('turns dash lines into a list, also right after their lead sentence', () => {
    expect(toBlocks('De hoogte:\n- a\n- b\nDus c.')).toEqual([
      { text: 'De hoogte:' },
      { items: ['a', 'b'] },
      { text: 'Dus c.' },
    ]);
  });

  it('copes with nothing yet', () => {
    expect(toBlocks('')).toEqual([]);
  });
});
