import { readFileSync, readdirSync } from 'fs';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { load } from 'js-yaml';
import { describe, expect, it } from 'vitest';
import { CORPUS_NAMES, KINDS, slides } from '../src/presentation/slides.js';
import { DEMO_CASUS, DEMO_SCENARIOS, demoGuides } from '../src/presentation/demo.js';
import { createPresentation, keyAction, startIndex } from '../src/presentation/usePresentation.js';

// The slides name reductions, open terms and outputs of the corpus, and
// quote it. Nothing else checks that those names and lines still exist.
const corpusDir = fileURLToPath(new URL('../../../../corpus', import.meta.url));
function yamlFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? yamlFiles(join(dir, e.name)) : e.name.endsWith('.yaml') ? [join(dir, e.name)] : [],
  );
}
const corpus = [join(corpusDir, 'regulation'), join(corpusDir, 'cells')]
  .flatMap(yamlFiles)
  .map((f) => readFileSync(f, 'utf8'))
  .join('\n');

const texts = (s) => [s.title, s.subtitle, s.lead, ...(s.lines ?? []), ...(s.bullets ?? [])].filter(Boolean);

describe('the slides', () => {
  it.each(slides.map((s, i) => [i + 1, s]))('slide %i has a known kind and a title', (_, s) => {
    expect(KINDS).toContain(s.kind);
    expect(s.title?.trim()).toBeTruthy();
  });

  it('end on the closing slide, the only one', () => {
    expect(slides.at(-1).kind).toBe('closing');
    expect(slides.filter((s) => s.kind === 'closing')).toHaveLength(1);
  });

  it.each(CORPUS_NAMES)('names %s, which the corpus has', (name) => {
    expect(corpus).toContain(name);
  });

  it('names nothing in code that the corpus does not have', () => {
    const named = slides
      .flatMap(texts)
      .flatMap((t) => [...t.matchAll(/`([^`]+)`/g)].map((m) => m[1]));
    expect(named.length).toBeGreaterThan(0);
    for (const n of named) expect(corpus, n).toContain(n);
  });

  it('quotes each YAML fragment from the file it names, in order', () => {
    const repo = fileURLToPath(new URL('../../../../', import.meta.url));
    const fragments = slides.map((s) => s.figure).filter((f) => f?.source);
    expect(fragments.length).toBeGreaterThan(0);
    for (const { code, source } of fragments) {
      const file = readFileSync(join(repo, source), 'utf8')
        .split('\n')
        .map((l) => l.trim());
      let at = 0;
      for (const line of code.split('\n')) {
        const t = line.trim();
        if (!t || t.startsWith('#')) continue;
        const found = file.indexOf(t, at);
        expect(found, `${source}: ${t}`).toBeGreaterThanOrEqual(0);
        at = found + 1;
      }
    }
  });

  it('names a source for every YAML fragment', () => {
    for (const s of slides.filter((x) => x.figure?.code && !x.figure.code.trimStart().startsWith('{')))
      expect(s.figure.source, s.title).toBeTruthy();
  });

  it('put the paper in words of their own, without quotes', () => {
    for (const s of slides) {
      for (const t of texts(s)) expect(t, s.title).not.toMatch(/[„”]/);
      expect(s.cite, s.title).toBeUndefined();
    }
  });

  it('keep a concept slide to five bullets', () => {
    for (const s of slides) expect(s.bullets?.length ?? 0, s.title).toBeLessThanOrEqual(5);
  });

  it('give each figure a label', () => {
    for (const s of slides.filter((x) => x.figure)) expect(s.figure.label, s.title).toBeTruthy();
  });
});

describe('the demo scenarios', () => {
  const yaml = load(
    readFileSync(new URL(`../../casussen/${DEMO_CASUS}.yaml`, import.meta.url), 'utf8'),
  );

  it('are scenarios of the case, each with a guide', () => {
    const names = yaml.scenarios.map((s) => s.naam);
    for (const n of DEMO_SCENARIOS) expect(names).toContain(n);
    expect(demoGuides().map((g) => g?.name)).toEqual([...DEMO_SCENARIOS]);
  });
});

describe('startIndex', () => {
  it('opens the deck only with the parameter, at the slide it names', () => {
    expect(startIndex('', slides.length)).toBeNull();
    expect(startIndex('?casus=x', slides.length)).toBeNull();
    expect(startIndex('?presentatie', slides.length)).toBe(0);
    const n = slides.length;
    expect(startIndex('?presentatie=5', n)).toBe(4);
    expect(startIndex('?presentatie=99', n)).toBe(n - 1);
    expect(startIndex('?presentatie=nee', n)).toBe(0);
  });
});

describe('keyAction', () => {
  const key = (k, target = {}) => ({ key: k, composedPath: () => [target] });
  const el = (selector) => ({ matches: (s) => s.split(', ').includes(selector) });

  it('turns the page with the arrows, Space and Page keys', () => {
    expect(keyAction(key('F'))).toBe('fullscreen');
    expect(keyAction(key('ArrowRight'))).toBe('next');
    expect(keyAction(key(' '))).toBe('next');
    expect(keyAction(key('PageUp'))).toBe('prev');
    expect(keyAction(key('Home'))).toBe('first');
    expect(keyAction(key('End'))).toBe('last');
    expect(keyAction(key('Escape'))).toBe('stop');
    expect(keyAction(key('x'))).toBeNull();
  });

  it('leaves a field and a focused button alone', () => {
    expect(keyAction(key('ArrowRight', el('input')))).toBeNull();
    expect(keyAction(key(' ', el('nldd-button')))).toBeNull();
    // An arrow does nothing on a button, so it still turns the page.
    expect(keyAction(key('ArrowRight', el('nldd-button')))).toBe('next');
  });

  it('leaves a key with a modifier to the browser', () => {
    expect(keyAction({ ...key('ArrowLeft'), altKey: true })).toBeNull();
    expect(keyAction({ ...key('f'), ctrlKey: true })).toBeNull();
    expect(keyAction({ ...key('ArrowRight'), metaKey: true })).toBeNull();
  });

  it('leaves Escape to an open dialog or popover', () => {
    expect(keyAction(key('Escape', el('dialog[open]')))).toBeNull();
  });
});

describe('createPresentation', () => {
  it('moves within the deck and stops', () => {
    const p = createPresentation(slides);
    p.start(0, { keys: false });
    expect(p.active.value).toBe(true);
    p.prev();
    expect(p.index.value).toBe(0);
    p.goTo(99);
    expect(p.isLast.value).toBe(true);
    p.next();
    expect(p.index.value).toBe(slides.length - 1);
    p.stop();
    expect(p.active.value).toBe(false);
  });
});

describe('back to the deck from a scenario', () => {
  it('returns to the slide the scenario was chosen on', () => {
    const p = createPresentation(slides);
    p.start(slides.length - 1, { keys: false });
    p.leave();
    expect(p.active.value).toBe(false);
    p.resume();
    expect(p.active.value).toBe(true);
    expect(p.index.value).toBe(slides.length - 1);
    p.stop();
  });

  it('starts at the first slide when the visitor did not come from the deck', () => {
    const p = createPresentation(slides);
    p.resume();
    expect(p.index.value).toBe(0);
    p.stop();
  });

  it('forgets nothing when the deck closes another way', () => {
    const p = createPresentation(slides);
    p.start(slides.length - 1, { keys: false });
    p.leave();
    p.resume();
    p.goTo(2);
    p.stop();
    p.resume();
    expect(p.index.value).toBe(slides.length - 1);
    p.stop();
  });
});
