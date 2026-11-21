// @vitest-environment node
// Where the citizen's entry sends someone who is in, and someone who is not,
// whatever the transport.
import { describe, expect, it } from 'vitest';
import type { RouteLocationNormalized } from 'vue-router';
import { omleiding } from './router';
import { BURGER, terugNaar } from '../gedeeld/terug';

const naar = (path: string, name?: string) => ({ path, fullPath: path, name }) as unknown as RouteLocationNormalized;
const binnen = { binnen: true, naam: 'Test Burger' };
const buiten = { binnen: false, naam: null };

describe('the routes of the citizen entry', () => {
  it('opens on the dossier for a citizen who is in, and on the way in otherwise', () => {
    expect(omleiding(naar('/'), binnen)).toBe('/burger');
    expect(omleiding(naar('/'), buiten)).toBe('/toegang');
  });

  it('keeps someone who is not in off the citizen screens', () => {
    expect(omleiding(naar('/burger', 'dossier'), buiten)).toBe('/toegang');
    expect(omleiding(naar('/burger/aanvraag', 'aanvraag'), buiten)).toEqual({ name: 'toegang', query: { terug: '/burger/aanvraag' } });
    expect(omleiding(naar('/toegang', 'toegang'), buiten)).toBeUndefined();
    expect(omleiding(naar('/burger/aanvraag', 'aanvraag'), binnen)).toBeUndefined();
  });

  it('sends someone back to the screen of a link after getting in, and nowhere else', () => {
    expect(terugNaar('/burger/besluit/01a1', BURGER, '/burger')).toBe('/burger/besluit/01a1');
    for (const elders of ['//voorbeeld.org/x', 'https://voorbeeld.org', '/backoffice', '/burgerX', undefined, ['/burger/x']]) {
      expect(terugNaar(elders, BURGER, '/burger')).toBe('/burger');
    }
  });
});
