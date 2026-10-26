/**
 * Het script in `index.html` kent dezelfde talen als de talentabel.
 *
 * Dat script draait vóór de bundle en kan dus niets importeren: het draagt zijn
 * eigen lijstje prefixen. Twee lijsten die hetzelfde horen te zeggen lopen
 * uiteen zodra iemand er één bijwerkt, en dit is een bijzonder stille variant.
 * Wie een taal toevoegt zonder het lijstje bij te werken, krijgt een demo die
 * gewoon werkt: alleen komt iemand die die taal koos op `/` in het Nederlands
 * binnen, en kondigt een anderstalige pagina zich bij een schermlezer als
 * Nederlands aan tot de router langskomt. Geen foutmelding, niets roods.
 *
 * Vandaar dat deze test het bestand terugleest in plaats van de regel te
 * vertrouwen.
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { DEFAULT_LOCALE, LOCALES } from './index.js';

const here = dirname(fileURLToPath(import.meta.url));
const html = readFileSync(resolve(here, '..', '..', 'index.html'), 'utf8');

/** De `PREFIXED`-literal uit index.html, als paren [code, prefix]. */
function prefixedFromHtml(): [string, string][] {
  const match = html.match(/var PREFIXED = (\[.*?\]);/s);
  if (!match) throw new Error('index.html heeft geen `var PREFIXED = [...]` meer; pas deze test aan');
  // The pattern has one group, so a match always has it.
  return JSON.parse(match[1]!.replaceAll("'", '"')) as [string, string][];
}

describe('het pre-boot script in index.html', () => {
  it('kent precies de geprefixte talen uit de tabel', () => {
    const expected = LOCALES.filter((l) => l.prefix).map((l) => [l.code, l.prefix]);
    expect(prefixedFromHtml()).toEqual(expected);
  });

  it('laat de bron erbuiten', () => {
    // De bron heeft geen prefix: `/` is zijn eigen adres. Stond hij in de lijst
    // met een lege prefix, dan zou `path.indexOf('' + '/') === 0` op élk pad
    // aanslaan en zou elke bezoeker als Nederlands worden aangemerkt.
    expect(prefixedFromHtml().map(([code]) => code)).not.toContain(DEFAULT_LOCALE);
  });

  it('gebruikt dezelfde opslagsleutel als de app', () => {
    expect(html).toContain("localStorage.getItem('rr-lang')");
  });
});
