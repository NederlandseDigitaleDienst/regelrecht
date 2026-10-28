import { existsSync, readFileSync } from 'fs';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { load } from 'js-yaml';
import { afterEach, describe, expect, it } from 'vitest';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import UitlegPagina from '../src/presentation/UitlegPagina.vue';
import { matches, nodeAt, nodePaths } from '@regelrecht/frontend-shared/yamlExpand';
import { bronnen, casus, intro, openPaden, steps } from '../src/presentation/uitleg.js';
import { asked, createUitleg, withParam } from '../src/presentation/useUitleg.js';

const repo = fileURLToPath(new URL('../../../../', import.meta.url));
const panels = steps.flatMap((s) => s.panels.map((p) => [`${s.title}: ${p.label}`, p]));
const texts = [intro.title, intro.lead, ...steps.flatMap((s) => [s.title, ...s.text])];

describe('the explanation page', () => {
  it.each(panels)('%s shows a node of its file, and every path that starts open is there', (_, p) => {
    const doc = load(readFileSync(join(repo, p.source), 'utf8'));
    expect(bronnen).toContain(p.source);
    const root = nodeAt(doc, p.root);
    expect(root, `${p.source}: ${p.root}`).toBeTruthy();
    expect(typeof root).toBe('object');
    expect(p.open.length).toBeGreaterThan(0);
    const paths = nodePaths(root, p.root);
    for (const pattern of openPaden(p)) expect(paths.some((q) => matches(pattern, q)), `${p.source}: ${pattern}`).toBe(true);
  });

  it.each(steps.filter((s) => s.quote).map((s) => [s.title, s.quote]))(
    '%s quotes the text of its article word for word',
    (_, { source, artikel, tekst }) => {
      const law = load(readFileSync(join(repo, source), 'utf8'));
      const article = law.articles.find((a) => String(a.number) === artikel);
      // As a reader sees it: one space between words, a link as its text.
      const plain = (t) =>
        String(t)
          .replace(/\[([^\]]+)\]\[[^\]]*\]/g, '$1')
          .replace(/\s+/g, ' ')
          .trim();
      const text = plain(article.text);
      let at = 0;
      for (const part of [].concat(tekst).join(' ').split('…').map(plain).filter(Boolean)) {
        const found = text.indexOf(part, at);
        expect(found, `${source} art. ${artikel}: ${part}`).toBeGreaterThanOrEqual(0);
        at = found + part.length;
      }
    },
  );

  it('gives every step a short heading, two or three short sentences and a panel', () => {
    for (const s of steps) {
      expect(s.title.split(/\s+/).length, s.title).toBeLessThanOrEqual(4);
      // "art." and a dot in code end no sentence.
      const plain = s.text.join(' ').replaceAll('art.', 'art').replace(/`[^`]*`/g, 'x');
      const sentences = plain.match(/[^.!?]+[.!?]/g) ?? [];
      expect(sentences.length, s.title).toBeGreaterThanOrEqual(2);
      expect(sentences.length, s.title).toBeLessThanOrEqual(3);
      expect(s.panels.length, s.title).toBeGreaterThan(0);
    }
  });

  it('names nothing in code that its files do not have', () => {
    const all = [...new Set(steps.flatMap((s) => s.panels.map((p) => p.source)))]
      .map((f) => readFileSync(join(repo, f), 'utf8'))
      .join('\n');
    const named = texts.flatMap((t) => [...t.matchAll(/`([^`]+)`/g)].map((m) => m[1]));
    expect(named.length).toBeGreaterThan(0);
    for (const n of named) expect(all, n).toContain(n.replace(/^source\./, ''));
  });

  it('follows one case, the business parking permit, from the rule to the chronicle', () => {
    expect(steps[0].title).toBe('Recht op een vergunning');
    expect(existsSync(join(repo, 'packages/poc-chronolexografie/casussen', `${casus}.yaml`))).toBe(true);
    const sources = steps.flatMap((s) => s.panels.map((p) => p.source)).join('\n');
    expect(sources).toContain('gemeente_wilgendaal/cel.yaml');
    expect(sources).toContain('reductie_geldige_bedrijfsparkeervergunningen');
    expect(sources).toContain('reductie_algemene_wet_bestuursrecht');
    expect(sources).toContain('kamer_van_koophandel');
    expect(`${texts.join(' ')} ${sources}`).not.toMatch(/oostmeer|cbr|gehandicapt/i);
  });
});

describe('the text of the page', () => {
  it('has no em dash or en dash as punctuation, in the data or the component', () => {
    const quoteLabels = steps.filter((s) => s.quote).map((s) => s.quote.label);
    const own = [...texts, ...panels.map(([, p]) => p.label), ...quoteLabels];
    for (const t of own) expect(t, t).not.toMatch(/[—–]/);
    const template = readFileSync(join(repo, 'packages/poc-chronolexografie/frontend/src/presentation/UitlegPagina.vue'), 'utf8')
      .split('<template>')[1];
    expect(template).not.toMatch(/[—–]/);
  });
});

describe('the address of the page', () => {
  it('is the page with ?uitleg, beside the paths of the cases', () => {
    expect(asked('?uitleg')).toBe(true);
    expect(asked('?presentatie=3')).toBe(false);
    expect(withParam('https://x.example/chronolexografie/', true)).toBe('https://x.example/chronolexografie/?uitleg');
    expect(withParam('https://x.example/chronolexografie/?uitleg', false)).toBe('https://x.example/chronolexografie/');
    expect(withParam('https://x.example/c/vakantieverlof?presentatie=2', true)).toBe(
      'https://x.example/c/vakantieverlof?presentatie=2&uitleg',
    );
  });

  it('opens and closes', () => {
    const u = createUitleg();
    u.open({ push: false });
    expect(u.active.value).toBe(true);
    u.close();
    expect(u.active.value).toBe(false);
  });
});

describe('the page', () => {
  const bestanden = Object.fromEntries(bronnen.map((f) => [f, readFileSync(join(repo, f), 'utf8')]));

  it('is a page of the app: a section per step, a tree per panel', async () => {
    const html = await renderToString(createSSRApp({ render: () => h(UitlegPagina, { bestanden }) }));
    expect(html.match(/<nldd-simple-section/g)).toHaveLength(steps.length + 2);
    expect(html).toContain('Naar de casus');
    expect(html.match(/class="yaml-tree"/g)).toHaveLength(panels.length);
    expect(html).toContain('Naar de casuskeuze');
    expect(html).toMatch(/<code[^>]*>aantal_geldige_bedrijfsparkeervergunningen<\/code>/);
    expect(html.match(/<nldd-blockquote lang="nl"/g)).toHaveLength(steps.filter((s) => s.quote).length);
    // The portal's strip says the regulation is invented; the page does not repeat it.
    expect(html).not.toMatch(/verzonnen/i);
    for (const t of [...texts, ...steps.filter((x) => x.quote).map((x) => x.quote.label)]) expect(t, t).not.toMatch(/fictie/i);
    expect(html).not.toContain('slot="attribution"');
  });

  it('opens the parts a step is about and leaves the rest folded', async () => {
    const html = await renderToString(createSSRApp({ render: () => h(UitlegPagina, { bestanden }) }));
    // The whole condition of article 2 of the regulation, down to the exception.
    expect(html).toContain('$weigeringsgrond_buiten_toepassing_gelaten');
    expect(html).toContain('$maximum_bedrijfsparkeervergunningen_bereikt');
    // Its inputs stay folded: the key shows, what is under it does not.
    expect(html).not.toContain('einde_geldigheidsduur_bij_verlening');
  });

  it('loads the files when none are given, and says so until they are there', async () => {
    const html = await renderToString(createSSRApp({ render: () => h(UitlegPagina) }));
    expect(html.match(/De YAML wordt geladen/g)).toHaveLength(panels.length);
  });
});

// A browser history of addresses, enough for useUitleg.js.
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

describe('the history of the page', () => {
  afterEach(() => {
    delete globalThis.window;
  });

  it('opens on an entry of its own and closes by going back over it', () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/'));
    const u = createUitleg();
    u.open();
    expect(w.location.search).toBe('?uitleg');
    u.close();
    expect(w.location.search).toBe('');
    expect(w.entries).toHaveLength(2);
    expect(u.active.value).toBe(false);
  });

  it('closes by going back also after back and forward', () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/'));
    const u = createUitleg();
    u.open();
    w.history.back();
    expect(u.active.value).toBe(false);
    w.history.forward();
    expect(u.active.value).toBe(true);
    u.close();
    expect(w.location.search).toBe('');
    expect(w.entries[1].url).toContain('?uitleg');
  });

  it('takes the parameter off when the page opened from a shared link', () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/?uitleg'));
    const u = createUitleg();
    u.follow();
    expect(u.active.value).toBe(true);
    u.close();
    expect(w.entries).toHaveLength(1);
    expect(w.location.search).toBe('');
    expect(u.terug.value).toBe(true);
  });

  it('gives the focus back after browser back, and takes it only after a click', () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/'));
    const u = createUitleg();
    u.open();
    expect(u.neemFocus()).toBe(true);
    expect(u.neemFocus()).toBe(false);
    w.history.back();
    expect(u.terug.value).toBe(true);
    const v = createUitleg();
    globalThis.window = fakeWindow('https://x.example/c/?uitleg');
    v.follow();
    expect(v.neemFocus()).toBe(false);
  });

  it('leaves without going back when a case is chosen', () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/vakantieverlof?uitleg'));
    const u = createUitleg();
    u.follow();
    u.verlaat();
    expect(u.active.value).toBe(false);
    expect(w.location.href).toBe('https://x.example/c/vakantieverlof');
  });

  it('goes to the case on a new entry, and back returns to the page', async () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/'));
    const u = createUitleg();
    u.open();
    await u.naarCasus(async () => {
      // As navigation.js writes it: the case's path with the search as it is.
      w.history.pushState(null, '', 'https://x.example/c/bedrijfsparkeervergunning' + w.location.search);
    });
    expect(w.location.href).toBe('https://x.example/c/bedrijfsparkeervergunning');
    expect(u.active.value).toBe(false);
    w.history.back();
    expect(w.location.search).toBe('?uitleg');
    expect(u.active.value).toBe(true);
  });

  it('gives way to a case named in the address', () => {
    const w = (globalThis.window = fakeWindow('https://x.example/c/vakantieverlof?uitleg'));
    const u = createUitleg();
    u.follow({ opCasus: true });
    expect(u.active.value).toBe(false);
    expect(w.location.href).toBe('https://x.example/c/vakantieverlof');
  });
});
