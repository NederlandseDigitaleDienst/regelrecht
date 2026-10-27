import { readFileSync, readdirSync } from 'fs';
import { fileURLToPath } from 'url';
import { load } from 'js-yaml';
import { describe, expect, it } from 'vitest';
import { findScenario, parseRoute, routePath, scenarioSlug } from '../src/route.js';

const casussenDir = fileURLToPath(new URL('../../casussen/', import.meta.url));
const casussen = readdirSync(casussenDir)
  .filter((f) => f.endsWith('.yaml'))
  .map((f) => ({ id: f.replace(/\.yaml$/, ''), ...load(readFileSync(casussenDir + f, 'utf8')) }));

describe('scenarioSlug', () => {
  it('writes a scenario name as a plain path segment', () => {
    expect(scenarioSlug('Twee uitgevers, één antwoord')).toBe('twee-uitgevers-een-antwoord');
    expect(scenarioSlug('Bezwaar te laat: niet-ontvankelijk')).toBe('bezwaar-te-laat-niet-ontvankelijk');
  });

  it.each(casussen.map((c) => [c.id, c]))('gives every scenario of %s its own slug', (_, c) => {
    const slugs = c.scenarios.map((s) => scenarioSlug(s.naam));
    expect(new Set(slugs).size).toBe(slugs.length);
    for (const slug of slugs) expect(slug).toMatch(/^[a-z0-9]+(-[a-z0-9]+)*$/);
  });
});

describe('routePath and parseRoute', () => {
  const base = '/chronolexografie/';

  it('write the choice screen, a case and a scenario', () => {
    expect(routePath(base)).toBe(base);
    expect(routePath(base, 'vakantieverlof')).toBe('/chronolexografie/vakantieverlof');
    expect(routePath(base, 'vakantieverlof', 'Aanvraag wordt verleend')).toBe(
      '/chronolexografie/vakantieverlof/aanvraag-wordt-verleend',
    );
  });

  it('read back what they write', () => {
    for (const c of casussen) {
      for (const s of c.scenarios) {
        const route = parseRoute(routePath(base, c.id, s.naam), base);
        expect(route).toEqual({ casus: c.id, scenario: scenarioSlug(s.naam), onbekend: false });
        expect(findScenario(c, route.scenario)).toBe(s);
      }
    }
  });

  it('read the base with and without its slash, and a trailing slash', () => {
    const leeg = { casus: null, scenario: null, onbekend: false };
    expect(parseRoute('/chronolexografie/', base)).toEqual(leeg);
    expect(parseRoute('/chronolexografie', base)).toEqual(leeg);
    expect(parseRoute('/chronolexografie/vakantieverlof/', base).casus).toBe('vakantieverlof');
    expect(parseRoute('/', '/')).toEqual(leeg);
  });

  it('name nothing for a path that is deeper or outside the base', () => {
    expect(parseRoute('/chronolexografie/a/b/c', base).onbekend).toBe(true);
    expect(parseRoute('/elders/', base).onbekend).toBe(true);
    expect(parseRoute('/chronolexografie/%E0', base).onbekend).toBe(true);
  });

  it('find no scenario for an unknown slug', () => {
    expect(findScenario(casussen[0], 'bestaat-niet')).toBeUndefined();
  });
});
