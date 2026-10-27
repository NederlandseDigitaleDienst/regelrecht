import { readFileSync } from 'fs';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { describe, expect, it } from 'vitest';
import { intro, steps } from '../src/presentation/uitleg.js';
import { asked, createUitleg, withParam } from '../src/presentation/useUitleg.js';

const repo = fileURLToPath(new URL('../../../../', import.meta.url));
const lines = (source) =>
  readFileSync(join(repo, source), 'utf8')
    .split('\n')
    .map((l) => l.trim());

// What a panel shows of its YAML: its lines without the mark of a cut.
const shown = (code) =>
  code
    .split('\n')
    .map((l) => l.trim())
    .filter((t) => t && !t.startsWith('#'));

const panels = steps.flatMap((s) => s.panels.map((p) => [`${s.title}: ${p.label}`, p]));
const texts = [intro.title, intro.lead, ...steps.flatMap((s) => [s.title, ...s.text])];

describe('the explanation page', () => {
  it.each(panels)('%s quotes its file, in order', (_, { code, source }) => {
    const file = lines(source);
    let at = 0;
    for (const t of shown(code)) {
      const found = file.indexOf(t, at);
      expect(found, `${source}: ${t}`).toBeGreaterThanOrEqual(0);
      at = found + 1;
    }
  });

  it.each(panels)('%s is a small piece: 2 to 15 lines, without comments', (_, { code }) => {
    const n = code.split('\n').length;
    expect(n).toBeGreaterThanOrEqual(2);
    expect(n).toBeLessThanOrEqual(15);
    for (const l of code.split('\n')) if (l.trim().startsWith('#')) expect(l.trim()).toBe('# …');
  });

  it('gives every step a short heading, two or three short sentences and a panel', () => {
    for (const s of steps) {
      expect(s.title.split(/\s+/).length, s.title).toBeLessThanOrEqual(5);
      const sentences = s.text.join(' ').match(/[^.!?]+[.!?]/g) ?? [];
      expect(sentences.length, s.title).toBeGreaterThanOrEqual(2);
      expect(sentences.length, s.title).toBeLessThanOrEqual(4);
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

  it('follows one case, the parking card, from the law to the chronicle', () => {
    expect(steps[0].title).toBe('BABW art. 49');
    const sources = steps.flatMap((s) => s.panels.map((p) => p.source)).join('\n');
    expect(sources).toContain('gemeente_wilgendaal/cel.yaml');
    expect(sources).toContain('reductie_geldige_gehandicaptenparkeerkaart');
    expect(sources).toContain('reductie_algemene_wet_bestuursrecht');
    expect(sources).toContain('ggd_wilgendaal');
    expect(`${texts.join(' ')} ${sources}`).not.toMatch(/oostmeer|cbr|bedrijfsparkeer/i);
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
    u.onKey({ key: 'Escape', preventDefault() {} });
    expect(u.active.value).toBe(false);
  });
});
