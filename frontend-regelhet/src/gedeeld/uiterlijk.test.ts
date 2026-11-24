// The accent colour the server names lands on <html> as an NLDD theme, and a
// failed call leaves the screens on the design system's own accent.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { laadUiterlijk, zetUiterlijk } from './uiterlijk';

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

  it("keeps the design system's own accent without one", () => {
    const root = document.createElement('html');
    zetUiterlijk({ accentkleur: 'rood' }, root);
    zetUiterlijk({ accentkleur: null }, root);
    expect(root.dataset.nlddTheme).toBeUndefined();
  });

  it('leaves the screens as they are when the server does not answer', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('Failed to fetch'))));
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.spyOn(console, 'error').mockImplementation(() => {});
    await laadUiterlijk();
    expect(document.documentElement.dataset.nlddTheme).toBeUndefined();
    expect(warn).toHaveBeenCalled();
  });
});
