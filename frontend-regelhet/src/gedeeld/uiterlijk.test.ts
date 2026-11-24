// The accent colour the server names lands on <html> as an NLDD theme, and a
// server that names none, or does not answer, leaves the screens donkerblauw.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { laadUiterlijk, titel, zetUiterlijk } from './uiterlijk';

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  delete document.documentElement.dataset.nlddTheme;
});

const antwoord = (body: unknown) =>
  vi.fn(async () => new Response(JSON.stringify(body), { status: 200, headers: { 'content-type': 'application/json' } }));

describe('the accent colour', () => {
  it('sets the theme the server names', async () => {
    const fetch = antwoord({ accentkleur: 'paars' });
    vi.stubGlobal('fetch', fetch);
    await laadUiterlijk();
    expect(document.documentElement.dataset.nlddTheme).toBe('paars');
    expect(String((fetch.mock.calls[0] as unknown[])[0])).toContain('/api/uiterlijk');
  });

  it('wears donkerblauw without one', () => {
    const root = document.createElement('html');
    zetUiterlijk({ accentkleur: 'rood' }, root);
    zetUiterlijk({ accentkleur: null }, root);
    expect(root.dataset.nlddTheme).toBe('donkerblauw');
  });

  it('wears donkerblauw when the server does not answer', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('Failed to fetch'))));
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.spyOn(console, 'error').mockImplementation(() => {});
    await laadUiterlijk();
    expect(document.documentElement.dataset.nlddTheme).toBe('donkerblauw');
    expect(warn).toHaveBeenCalled();
  });
});

describe('the title', () => {
  it('is the one the server names, and none on an answer without it', () => {
    const root = document.createElement('html');
    zetUiterlijk({ accentkleur: null, titel: 'Gehandicaptenparkeerkaart' }, root);
    expect(titel.value).toBe('Gehandicaptenparkeerkaart');
    zetUiterlijk({ accentkleur: null }, root);
    expect(titel.value).toBeNull();
  });
});
