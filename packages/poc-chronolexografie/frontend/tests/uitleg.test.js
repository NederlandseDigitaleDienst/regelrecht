import { existsSync, readFileSync } from 'fs';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { load } from 'js-yaml';
import { afterEach, describe, expect, it } from 'vitest';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import UitlegPagina from '../src/presentation/UitlegPagina.vue';
import { casus, intro, steps } from '../src/presentation/uitleg.js';
import { asked, createUitleg, withParam } from '../src/presentation/useUitleg.js';

const repo = fileURLToPath(new URL('../../../../', import.meta.url));
const indent = (l) => l.length - l.trimStart().length;
const isMark = (l) => l.trim() === '# …';

// The lines of a file that hold YAML: no blank lines, no comment lines.
const yamlLines = (source) =>
  readFileSync(join(repo, source), 'utf8')
    .split('\n')
    .filter((l) => l.trim() && !l.trim().startsWith('#'));

// A panel in pieces: the runs of lines between the marks of a cut (`# …`).
function pieces(code) {
  const out = [[]];
  for (const l of code.split('\n')) {
    if (isMark(l)) out.push([]);
    else out[out.length - 1].push(l);
  }
  return out.filter((p) => p.length);
}

/**
 * Where a piece stands in `file` from line `from` on: the lines follow each
 * other there, each with the same indentation relative to the first, as the
 * panel shows them. -1 when nowhere.
 */
function find(file, piece, from) {
  for (let i = from; i + piece.length <= file.length; i++) {
    const shift = indent(file[i]) - indent(piece[0]);
    if (piece.every((l, j) => file[i + j].trim() === l.trim() && indent(file[i + j]) - indent(l) === shift)) return i;
  }
  return -1;
}

const panels = steps.flatMap((s) => s.panels.map((p) => [`${s.title}: ${p.label}`, p]));
const texts = [intro.title, intro.lead, ...steps.flatMap((s) => [s.title, ...s.text])];

describe('the explanation page', () => {
  it.each(panels)('%s quotes its file: runs of lines as written, cuts marked', (_, { code, source }) => {
    const file = yamlLines(source);
    const runs = pieces(code);
    let at = 0;
    let first = -1;
    let end = -1;
    for (const run of runs) {
      const i = find(file, run, at);
      expect(i, `${source}: ${run[0].trim()}`).toBeGreaterThanOrEqual(0);
      if (first < 0) first = i;
      end = i + run.length;
      at = end;
    }
    // A block that goes on after the panel ends in a mark, so nothing looks
    // complete that is not.
    const startIndent = indent(file[first]);
    const next = file[end];
    if (next && indent(next) > startIndent) expect(isMark(code.split('\n').at(-1)), `${source}: ends before ${next.trim()}`).toBe(true);
  });

  it.each(steps.filter((s) => s.quote).map((s) => [s.title, s.quote]))(
    '%s quotes the text of its article word for word',
    (_, { source, artikel, tekst, fictief }) => {
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
      expect(text.startsWith('FICTIEF'), `${source} art. ${artikel}`).toBe(!!fictief);
    },
  );

  it.each(panels)('%s is a small piece: 3 to 15 lines, without comments', (_, { code }) => {
    const n = code.split('\n').length;
    expect(n).toBeGreaterThanOrEqual(3);
    expect(n).toBeLessThanOrEqual(15);
    for (const l of code.split('\n')) if (l.trim().startsWith('#')) expect(l.trim()).toBe('# …');
  });

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
  it('is a page of the app: a section per step, a code viewer per panel', async () => {
    const html = await renderToString(createSSRApp({ render: () => h(UitlegPagina) }));
    expect(html.match(/<nldd-simple-section/g)).toHaveLength(steps.length + 2);
    expect(html).toContain('Naar de casus');
    expect(html.match(/<nldd-code-viewer[^>]*language="yaml"/g)).toHaveLength(panels.length);
    expect(html).toContain('Naar de casuskeuze');
    expect(html).toContain('<code>aantal_geldige_bedrijfsparkeervergunningen</code>');
    expect(html.match(/<nldd-blockquote lang="nl"/g)).toHaveLength(steps.filter((s) => s.quote).length);
    expect(html).toContain('verzonnen voor deze PoC');
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
