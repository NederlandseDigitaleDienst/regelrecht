// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { ALLE_KANTEN, BURGER, terugNaar } from './terug';

describe('terugNaar', () => {
  it("leads back to a screen of the build's own sides only", () => {
    expect(terugNaar('/backoffice/kroniek/c', ALLE_KANTEN, '')).toBe('/backoffice/kroniek/c');
    expect(terugNaar('/backoffice/kroniek/c', BURGER, '/burger')).toBe('/burger');
    for (const elders of ['//voorbeeld.org/x', 'https://voorbeeld.org', '/inloggen', '/burgerX', undefined, ['/burger/x']]) {
      expect(terugNaar(elders, ALLE_KANTEN, '')).toBe('');
    }
  });
});
