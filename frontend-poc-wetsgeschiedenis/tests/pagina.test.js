// Wat deze poc achter het portaal nodig heeft en wat geen andere test ziet:
// de CSP laat geen inline script toe, en de pagina haalt haar gegevens op
// onder het pad van de poc, niet vanaf de wortel van het domein.
import { describe, expect, test } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const app = resolve(__dirname, '..');
const lees = (pad) => readFileSync(resolve(app, pad), 'utf8');
const gegevens = resolve(app, '..', 'corpus-poc', 'wetsgeschiedenis', 'gegevens');

describe('de pagina', () => {
  test('heeft geen inline script: elk script-element laadt een bestand', () => {
    const html = lees('src/index.html');
    const scripts = [...html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/g)];
    expect(scripts.length).toBeGreaterThan(0);
    for (const [, attrs, body] of scripts) {
      expect(attrs).toMatch(/\bsrc=/);
      expect(body.trim()).toBe('');
    }
    expect(html).not.toMatch(/\son[a-z]+=/);
  });

  test('haalt de gegevens op onder de basis van de bundel', () => {
    const js = lees('src/pagina.js');
    expect(js).toContain('const BASIS = import.meta.env.BASE_URL;');
    expect(js).not.toMatch(/fetch\(\s*["'`]\//);
  });
});

describe('de gegevens', () => {
  const dossiers = JSON.parse(readFileSync(resolve(gegevens, 'dossiers.json'), 'utf8'));

  test('noemen ten minste één dossier', () => {
    expect(dossiers.length).toBeGreaterThan(0);
  });

  test.each(dossiers.map((d) => [d.dossier, d.naam]))('%s (%s) is er en speelt af', (id) => {
    const pad = resolve(gegevens, `${id}.json`);
    expect(existsSync(pad)).toBe(true);
    const t = JSON.parse(readFileSync(pad, 'utf8'));
    expect(t.dossier).toBe(id);
    expect(t.stappen.length).toBeGreaterThan(0);
    expect(t.momenten.length).toBeGreaterThan(0);
  });
});
