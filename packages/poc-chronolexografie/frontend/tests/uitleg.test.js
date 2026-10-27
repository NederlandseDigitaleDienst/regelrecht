import { readFileSync } from 'fs';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { describe, expect, it } from 'vitest';
import { intro, steps } from '../src/presentation/uitleg.js';
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
