// @vitest-environment node
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { articleContext, makeVocabulary, normaliseAction, renderAction, type ArticleContext } from './index';
import { isRecord } from './types';
import { corpusActions } from './test-support/corpus';
import { GLOSSARY, UNIT_DECLARATIONS, generateAction, rng } from './test-support/generate';
import { parseActionAll } from './test-support/parse';

const repo = (p: string) => fileURLToPath(new URL(`../../../../${p}`, import.meta.url));

// JSON drops what YAML never has (undefined) and fixes nothing else.
const plain = (v: unknown): unknown => JSON.parse(JSON.stringify(v));

function expected(action: unknown): unknown {
  const n = normaliseAction(action);
  delete n.resolve;
  return plain(n);
}

describe.each(['corpus/regulation'])('round trip over %s', (dir) => {
  const actions = corpusActions(repo(dir));

  it('has actions to read', () => {
    expect(actions.length).toBeGreaterThan(100);
  });

  it('reads every action back to its normalised tree, with exactly one reading', () => {
    const failures: string[] = [];
    for (const { file, article, action } of actions) {
      const output = isRecord(action) ? action.output : undefined;
      const ctx = articleContext(article);
      const { text } = renderAction(action, { article, ctx });
      const parses = parseActionAll(text, ctx);
      if (parses.length !== 1) {
        failures.push(`${file} ${output}: ${parses.length} readings`);
        continue;
      }
      try {
        expect(plain(parses[0])).toEqual(expected(action));
      } catch {
        failures.push(`${file} ${output}: different tree`);
      }
    }
    expect(failures).toEqual([]);
  });
});

describe('generated trees', () => {
  const ctx: ArticleContext = { declared: UNIT_DECLARATIONS, vocab: makeVocabulary(GLOSSARY) };
  const SEEDS = 3000;

  it('round-trip, with exactly one reading each', () => {
    const failures: string[] = [];
    for (let seed = 1; seed <= SEEDS; seed++) {
      const action = generateAction(rng(seed), 4);
      const { text } = renderAction(action, { ctx });
      const parses = parseActionAll(text, ctx);
      if (parses.length !== 1) {
        failures.push(`seed ${seed}: ${parses.length} readings`);
        continue;
      }
      try {
        expect(plain(parses[0])).toEqual(expected(action));
      } catch {
        failures.push(`seed ${seed}: different tree`);
      }
    }
    expect(failures).toEqual([]);
  });

  it('never give two different normalised trees the same text', () => {
    const byText = new Map<string, string>();
    const collisions: number[] = [];
    for (let seed = 1; seed <= SEEDS; seed++) {
      const action = generateAction(rng(seed), 3);
      const { text } = renderAction(action, { ctx });
      const tree = JSON.stringify(expected(action));
      const seen = byText.get(text);
      if (seen !== undefined && seen !== tree) collisions.push(seed);
      byText.set(text, tree);
    }
    expect(collisions).toEqual([]);
  });
});

describe('determinism', () => {
  const ctx: ArticleContext = { declared: UNIT_DECLARATIONS, vocab: makeVocabulary(GLOSSARY) };

  // Reverses the key order of every object, recursively.
  const shuffle = (v: unknown): unknown => {
    if (Array.isArray(v)) return v.map(shuffle);
    if (v === null || typeof v !== 'object') return v;
    const src = v as Record<string, unknown>;
    const out: Record<string, unknown> = {};
    for (const k of Object.keys(src).reverse()) out[k] = shuffle(src[k]);
    return out;
  };

  it('gives the same bytes for the same tree, whatever the key order', () => {
    for (let seed = 1; seed <= 500; seed++) {
      const action = generateAction(rng(seed), 4);
      const a = renderAction(action, { ctx }).text;
      const b = renderAction(structuredClone(action), { ctx }).text;
      const c = renderAction(shuffle(action), { ctx }).text;
      expect(b).toBe(a);
      expect(c).toBe(a);
    }
  });
});
