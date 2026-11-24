// @vitest-environment node
// The public side stays apart from the back office, so it is bundled on its
// own in the citizen's entry (src/burgeromgeving/, burger.html), which the app
// builds with another way in: no module
// of burger/ imports one of backoffice/ or names a back-office route, and none
// says how a request proves who sends it (that is gedeeld/sessie.ts). What
// both sides share, in gedeeld/, imports neither side. And the citizen's
// entry, every module it reaches with either transport (`#vervoer`), holds
// nothing of the back office; with the app's transport nothing of the login
// either, and nothing that keeps data in the webview's storage. The back
// office has its own entry (src/backofficeomgeving/, backoffice.html): no
// module of backoffice/ imports one of burger/ or names a citizen's route,
// and nothing the entry reaches is of the public side. The front of an
// organisation has its own entry (src/organisatieomgeving/, organisatie.html):
// it reaches none of a process's screens, and its modules name no route of a
// process's API directly: they reach a process only through the front's relay
// (`/api/processen/{id}/...`). The combined entry
// (main.ts) is the only one that carries both.
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
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

describe('the back office', () => {
  const backoffice = bestanden(join(src, 'backoffice'));

  it('has modules to check', () => {
    expect(backoffice.length).toBeGreaterThan(3);
  });

  it.each(backoffice.map((p) => [relative(src, p), p]))('%s imports nothing of the public side', (_, pad) => {
    for (const m of imports(pad)) {
      expect(m, m).not.toMatch(/(^|\/)(burger|burgeromgeving)\//);
    }
  });

  it.each(backoffice.map((p) => [relative(src, p), p]))("%s names no citizen's route", (_, pad) => {
    const tekst = readFileSync(pad, 'utf8');
    expect(tekst).not.toMatch(/\/api\/(dossier|indieningen)|\/fsc4b\/|['"`]\/burger/);
  });
});

describe('what both sides share', () => {
  it.each(bestanden(join(src, 'gedeeld')).map((p) => [relative(src, p), p]))('%s imports neither side', (_, pad) => {
    for (const m of imports(pad)) expect(m, m).not.toMatch(/\.\.\/(burger|backoffice|inloggen)\//);
  });
});

/** `m` as imported from `pad`, as a file, or null when it is a package. */
function resolve(pad: string, m: string, vervoer: string): string | null {
  if (m === '#vervoer') return join(src, 'burgeromgeving', 'vervoer', `${vervoer}.ts`);
  if (!m.startsWith('.')) return null;
  const basis = join(dirname(pad), m);
  for (const kandidaat of [basis, `${basis}.ts`, `${basis}.vue`, join(basis, 'index.ts')]) {
    if (existsSync(kandidaat) && statSync(kandidaat).isFile()) return kandidaat;
  }
  throw new Error(`${relative(src, pad)} imports ${m}, which does not resolve`);
}

/** Every module of src/ the entry `entry` (a path under src/) reaches with the transport `vervoer`, as paths under src/. */
function bundel(vervoer: string, entry = join('burgeromgeving', 'main.ts')): string[] {
  const gezien = new Set<string>();
  const te = [join(src, entry)];
  while (te.length) {
    const pad = te.pop() as string; // the loop runs while `te` is not empty
    if (gezien.has(pad)) continue;
    gezien.add(pad);
    for (const m of imports(pad)) {
      const doel = resolve(pad, m, vervoer);
      if (doel) te.push(doel);
    }
  }
  return [...gezien].map((p) => relative(src, p));
}

describe('the citizen entry with the transport of the app', () => {
  const app = bundel('tauri');

  it('reaches the citizen screens and the core session', () => {
    expect(app).toContain(join('burger', 'routes.ts'));
    expect(app).toContain(join('burger', 'AanvraagView.vue'));
    expect(app).toContain(join('burgeromgeving', 'tauri', 'kern.ts'));
  });

  it('reaches nothing of the back office, the login or the web shell', () => {
    for (const m of app) {
      expect(m, m).not.toMatch(/^(backoffice|inloggen)\//);
      expect(m, m).not.toMatch(/^(App\.vue|router\.ts|main\.ts|start\.ts)$/);
      expect(m, m).not.toBe(join('gedeeld', 'ik.ts'));
      expect(m, m).not.toBe(join('burgeromgeving', 'vervoer', 'cookie.ts'));
    }
  });

  it.each(app.map((m) => [m]))('%s keeps nothing in the storage of the webview', (m) => {
    expect(readFileSync(join(src, m), 'utf8')).not.toMatch(/localStorage|sessionStorage|indexedDB|document\.cookie/);
  });
});

describe('the citizen entry with the cookie transport', () => {
  const web = bundel('cookie');

  it('reaches the citizen screens and the login, and nothing of the back office or the app', () => {
    expect(web).toContain(join('burger', 'routes.ts'));
    expect(web).toContain(join('inloggen', 'InloggenView.vue'));
    for (const m of web) {
      expect(m, m).not.toMatch(/^backoffice\//);
      expect(m, m).not.toMatch(/^burgeromgeving\/(tauri\/|vervoer\/tauri)/);
    }
  });
});

describe('the back-office entry', () => {
  const backoffice = bundel('cookie', join('backofficeomgeving', 'main.ts'));

  it('reaches the back-office screens, the login and the shell', () => {
    expect(backoffice).toContain(join('backoffice', 'routes.ts'));
    expect(backoffice).toContain(join('backoffice', 'WerkvoorraadView.vue'));
    expect(backoffice).toContain(join('inloggen', 'InloggenView.vue'));
    expect(backoffice).toContain('App.vue');
  });

  it('reaches nothing of the public side or the app', () => {
    for (const m of backoffice) {
      expect(m, m).not.toMatch(/^(burger|burgeromgeving)\//);
      expect(m, m).not.toBe('main.ts');
    }
  });
});

describe("the entry of an organisation's front", () => {
  const front = bundel('cookie', join('organisatieomgeving', 'main.ts'));

  it('reaches the tiles, the login and the shell', () => {
    expect(front).toContain(join('organisatie', 'OrganisatieHome.vue'));
    expect(front).toContain(join('inloggen', 'InloggenView.vue'));
    expect(front).toContain('App.vue');
  });

  it("reaches none of a process's screens", () => {
    for (const m of front) {
      expect(m, m).not.toMatch(/^(burger|burgeromgeving|backoffice)\//);
      expect(m, m).not.toBe('main.ts');
    }
  });

  it.each(bestanden(join(src, 'organisatie')).map((p) => [relative(src, p), p]))(
    "%s names no route of a process's API",
    (_, pad) => {
      expect(readFileSync(pad, 'utf8')).not.toMatch(/\/api\/(backoffice|dossier|indieningen|casus|formulier)/);
    },
  );
});

describe('the combined entry', () => {
  // The largest graph to walk, both sides at once: on a busy machine it takes
  // longer than the five seconds a test gets by default.
  it('is the one that reaches both sides', { timeout: 20_000 }, () => {
    const beide = bundel('cookie', 'main.ts');
    expect(beide).toContain(join('burger', 'routes.ts'));
    expect(beide).toContain(join('backoffice', 'routes.ts'));
  });
});
