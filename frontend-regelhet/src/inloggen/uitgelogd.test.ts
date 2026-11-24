import { afterEach, describe, expect, it, vi } from 'vitest';
import { markeerUitgelogd, wasUitgelogd } from './uitgelogd';

afterEach(() => {
  vi.restoreAllMocks();
  sessionStorage.clear();
});

describe('the logout this tab remembers', () => {
  it('is told once', () => {
    expect(wasUitgelogd()).toBe(false);
    markeerUitgelogd();
    expect(wasUitgelogd()).toBe(true);
    expect(wasUitgelogd()).toBe(false);
  });

  it('is never there without storage, and never throws', () => {
    vi.spyOn(sessionStorage, 'setItem').mockImplementation(() => {
      throw new Error('geblokkeerd');
    });
    vi.spyOn(sessionStorage, 'getItem').mockImplementation(() => {
      throw new Error('geblokkeerd');
    });
    expect(() => markeerUitgelogd()).not.toThrow();
    expect(wasUitgelogd()).toBe(false);
  });
});
