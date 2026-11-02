import { existsSync, readFileSync } from 'fs';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { afterEach, describe, expect, it } from 'vitest';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import KortPagina from '../src/presentation/KortPagina.vue';
import CasusKeuze from '../src/components/CasusKeuze.vue';
import { casus, casusBlok, chronolexografie, intro, regelrecht, schema, verder } from '../src/presentation/kort.js';
import { KORT, createKort } from '../src/presentation/useKort.js';
import { asked, createUitleg, withParam } from '../src/presentation/useUitleg.js';

const repo = fileURLToPath(new URL('../../../../', import.meta.url));
const corpus = join(repo, 'packages/poc-chronolexografie/corpus');
const lijsten = [regelrecht, chronolexografie];
const texts = [
  intro.title,
  intro.lead,
  schema.bijschrift,
  casusBlok.title,
  casusBlok.lead,
  ...casusBlok.stappen,
  casusBlok.knop,
  ...lijsten.flatMap((l) => [l.title, ...l.bullets]),
  verder,
  ...schema.lagen.map((l) => l.label),
  ...schema.vakken.flatMap((v) => [v.titel, v.sub]),
];

describe('the short page', () => {
  it('numbers the arrows of the picture with the steps of the case', () => {
    const nummers = new Set(schema.pijlen.filter((p) => p.nummer).map((p) => p.nummer));
    expect([...nummers].sort((a, b) => a - b)).toEqual(casusBlok.stappen.map((_, i) => i + 1));
    for (const p of schema.pijlen.filter((q) => q.nummer)) expect(p.badge, `${p.nummer}`).toHaveLength(2);
  });

  it('follows a case the app has, with the files its steps name', () => {
    const file = join(corpus, 'casussen', `${casus}.yaml`);
    expect(existsSync(file)).toBe(true);
    const files = [
      file,
      join(corpus, 'cells/gemeente_wilgendaal/cel.yaml'),
      join(corpus, 'regulation/nl/uitvoeringsbeleid/beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal/2026-01-01.yaml'),
      join(corpus, 'cells/gemeente_wilgendaal/reductions/reductie_geldige_gehandicaptenparkeerkaart/2026-01-01.yaml'),
    ];
    const all = files.map((f) => readFileSync(f, 'utf8')).join('\n');
    const named = casusBlok.stappen.flatMap((t) => [...t.matchAll(/`([^`]+)`/g)].map((m) => m[1]));
    expect(named.length).toBeGreaterThan(0);
    for (const n of named) {
      if (n.endsWith('.yaml')) expect(existsSync(join(corpus, 'cells/gemeente_wilgendaal', n)), n).toBe(true);
      else expect(all, n).toContain(n.replace(/^source\./, ''));
    }
    // The answer of step 7 is the one the policy asks for in step 6.
    expect(all).toContain('aanvrager_heeft_geldige_gehandicaptenparkeerkaart: reductie_geldige_gehandicaptenparkeerkaart');
  });

  it('says that Wilgendaal is fictional', () => {
    expect(casusBlok.lead).toMatch(/Wilgendaal is een fictieve gemeente/);
  });

  it('keeps each list short', () => {
    for (const l of lijsten) {
      expect(l.bullets.length, l.title).toBeGreaterThanOrEqual(4);
      expect(l.bullets.length, l.title).toBeLessThanOrEqual(6);
    }
  });

  it('has no em dash or en dash as punctuation, in the data or the components', () => {
    for (const t of texts) expect(t, t).not.toMatch(/[—–]/);
    for (const f of ['KortPagina.vue', 'KortSchema.vue']) {
      const template = readFileSync(join(repo, 'packages/poc-chronolexografie/frontend/src/presentation', f), 'utf8').split('<template>')[1];
      expect(template, f).not.toMatch(/[—–]/);
    }
  });

  it('is a page of the app: the two lists, the picture, the case and the way on', async () => {
    const html = await renderToString(createSSRApp({ render: () => h(KortPagina) }));
    expect(html.match(/<nldd-simple-section/g)).toHaveLength(6);
    expect(html).toContain('Naar de casuskeuze');
    // The picture, described by its caption.
    expect(html).toMatch(/role="img" aria-describedby="kort-bijschrift"/);
    expect(html).toMatch(/<nldd-text id="kort-bijschrift"/);
    expect(html.match(/class="badge"/g)).toHaveLength(schema.pijlen.filter((p) => p.nummer).length);
    // The order: RegelRecht, chronolexografie, picture, case, the explanation page.
    const at = (s) => html.indexOf(s);
    expect(at(regelrecht.title)).toBeLessThan(at(chronolexografie.title));
    expect(at(chronolexografie.title)).toBeLessThan(at('kort-bijschrift'));
    expect(at('kort-bijschrift')).toBeLessThan(at(casusBlok.title));
    expect(at(casusBlok.title)).toBeLessThan(at(verder));
    expect(html.match(/<li[ >]/g)).toHaveLength(casusBlok.stappen.length + regelrecht.bullets.length + chronolexografie.bullets.length);
    expect(html).toMatch(/<code[^>]*>reductie_geldige_gehandicaptenparkeerkaart<\/code>/);
  });

  it('links to the case and to the explanation page', async () => {
    const html = await renderToString(createSSRApp({ render: () => h(KortPagina) }));
    expect(html).toMatch(new RegExp(`<nldd-button[^>]*href="/${casus}"[^>]*text="${casusBlok.knop}"`));
    expect(html).toMatch(new RegExp(`<nldd-button[^>]*href="/\\?uitleg"[^>]*text="${verder}"`));
    expect(verder).toBe('Lees de uitgebreide uitleg');
  });

  it('is what the choice screen links to, in place of the explanation page', async () => {
    const html = await renderToString(createSSRApp({ render: () => h(CasusKeuze) }));
    expect(html).toContain('text="In het kort"');
    expect(html).not.toContain('Hoe het in elkaar zit');
  });
});

describe('the address of the short page', () => {
  it('is the page with ?kort, beside the explanation page and the deck', () => {
    expect(asked('?kort', KORT)).toBe(true);
    expect(asked('?uitleg', KORT)).toBe(false);
    expect(asked('?kort')).toBe(false);
    expect(withParam('https://x.example/chronolexografie/', true, KORT)).toBe('https://x.example/chronolexografie/?kort');
    expect(withParam('https://x.example/chronolexografie/?kort', false, KORT)).toBe('https://x.example/chronolexografie/');
    expect(withParam('https://x.example/c/?presentatie=2', true, KORT)).toBe('https://x.example/c/?presentatie=2&kort');
  });
});

// A browser history of addresses, enough for useUitleg.js (as in uitleg.test.js).
function fakeWindow(start) {
  const entries = [{ url: start, state: null }];
  let at = 0;
  const listeners = [];
  const w = {
    get location() {
      const u = new URL(entries[at].url);
      return { href: u.href, search: u.search };
    },
    history: {
      get state() {
        return entries[at].state;
      },
      pushState(state, _, url) {
        entries.splice(at + 1);
        entries.push({ url, state });
        at += 1;
      },
      replaceState(state, _, url) {
        entries[at] = { url, state };
      },
      back() {
        at -= 1;
        listeners.forEach((f) => f());
      },
      forward() {
        at += 1;
        listeners.forEach((f) => f());
      },
    },
    addEventListener: (type, f) => type === 'popstate' && listeners.push(f),
    entries,
  };
  return w;
}

describe('the history of the short page', () => {
  afterEach(() => {
    delete globalThis.window;
  });

  it('opens on an entry of its own and closes by going back over it', () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/'));
    const k = createKort();
    k.open();
    expect(w.location.search).toBe('?kort');
    k.close();
    expect(w.location.search).toBe('');
    expect(k.active.value).toBe(false);
  });

  it('hands its entry to the explanation page, so back and close return to the choice screen', () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/'));
    const k = createKort();
    const u = createUitleg();
    k.open();
    u.open({ vervangt: KORT });
    k.verlaat();
    expect(w.entries).toHaveLength(2);
    expect(w.location.search).toBe('?uitleg');
    expect(k.active.value).toBe(false);
    expect(u.active.value).toBe(true);
    expect(u.neemFocus()).toBe(true);
    u.close();
    expect(w.location.search).toBe('');
    expect(u.active.value).toBe(false);
    expect(k.active.value).toBe(false);
  });

  it('does not go back out of the app when the short page came from a shared link', () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/?kort'));
    const k = createKort();
    const u = createUitleg();
    k.follow();
    expect(k.active.value).toBe(true);
    u.open({ vervangt: KORT });
    k.verlaat();
    expect(w.location.search).toBe('?uitleg');
    u.close();
    expect(w.entries).toHaveLength(1);
    expect(w.location.search).toBe('');
  });

  it('gives way to a case named in the address', () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/vakantieverlof?kort'));
    const k = createKort();
    k.follow({ opCasus: true });
    expect(k.active.value).toBe(false);
    expect(w.location.href).toBe('https://x.example/c/vakantieverlof');
  });
});
