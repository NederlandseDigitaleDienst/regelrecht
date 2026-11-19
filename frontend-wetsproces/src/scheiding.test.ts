// @vitest-environment node
// The public side stays apart from the back office, so it can later be
// bundled on its own, such as in an app that logs in another way: no module
// of burger/ imports one of backoffice/ or names a back-office route, and none
// says how a request proves who sends it (that is gedeeld/sessie.ts). What
// both sides share, in gedeeld/, imports neither side.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const src = dirname(fileURLToPath(import.meta.url));

function bestanden(map: string): string[] {
  return readdirSync(map).flatMap((naam) => {
    const pad = join(map, naam);
    if (statSync(pad).isDirectory()) return bestanden(pad);
    return /\.(ts|vue)$/.test(naam) && !naam.endsWith('.test.ts') ? [pad] : [];
  });
}

/** The modules `pad` imports, as written. */
const imports = (pad: string): string[] =>
  [...readFileSync(pad, 'utf8').matchAll(/(?:from|import)\s*\(?\s*['"]([^'"]+)['"]/g)].map((m) => m[1] ?? '');

describe('the public side', () => {
  const burger = bestanden(join(src, 'burger'));

  it('has modules to check', () => {
    expect(burger.length).toBeGreaterThan(3);
  });

  it.each(burger.map((p) => [relative(src, p), p]))('%s imports nothing of the back office or the login', (_, pad) => {
    for (const m of imports(pad)) {
      expect(m, m).not.toMatch(/backoffice|inloggen/);
    }
  });

  it.each(burger.map((p) => [relative(src, p), p]))('%s names no back-office route and no cookie', (_, pad) => {
    const tekst = readFileSync(pad, 'utf8');
    expect(tekst).not.toContain('/api/backoffice');
    expect(tekst).not.toMatch(/document\.cookie|credentials/);
  });
});

describe('what both sides share', () => {
  it.each(bestanden(join(src, 'gedeeld')).map((p) => [relative(src, p), p]))('%s imports neither side', (_, pad) => {
    for (const m of imports(pad)) expect(m, m).not.toMatch(/\.\.\/(burger|backoffice|inloggen)\//);
  });
});
