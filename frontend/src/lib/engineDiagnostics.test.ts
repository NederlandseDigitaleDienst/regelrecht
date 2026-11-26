import { describe, it, expect } from 'vitest';
import { articlesOf, diagnosticsOf, placeLabel, ruleText, type EngineDiagnostic } from './engineDiagnostics';

const base: EngineDiagnostic = {
  severity: 'error', kind: 'type_check', law_id: 't', article: '2', place: null, position: null, rule: 'N1', message: 'm',
};

describe('diagnosticsOf', () => {
  it('reads the records off what the engine threw', () => {
    expect(diagnosticsOf({ error: 'x', diagnostics: [base] })).toEqual([base]);
  });

  it('has none for a bare string or a malformed list', () => {
    expect(diagnosticsOf('Failed to load law: x')).toEqual([]);
    expect(diagnosticsOf({ diagnostics: 'nope' })).toEqual([]);
    expect(diagnosticsOf({ diagnostics: [null, { rule: 'N1' }] })).toEqual([]);
  });
});

describe('placeLabel', () => {
  it('names each kind of place', () => {
    expect(placeLabel({ kind: 'output', name: 'hoogte' })).toBe('Uitkomst hoogte');
    expect(placeLabel({ kind: 'input', name: 'inkomen' })).toBe('Invoer inkomen');
    expect(placeLabel({ kind: 'definition', name: 'GRENS' })).toBe('Definitie GRENS');
    expect(placeLabel({ kind: 'action', index: 3 })).toBe('Actie 3');
  });

  it('has no label for the article as a whole', () => {
    expect(placeLabel({ kind: 'article' })).toBeNull();
    expect(placeLabel(null)).toBeNull();
  });
});

describe('ruleText', () => {
  it('has a sentence for every rule of the type check', () => {
    for (const rule of ['N1', 'N2', 'N3', 'N4', 'N5', 'T1', 'T2', 'T3', 'T4', 'T5']) {
      expect(ruleText({ ...base, rule }), rule).toBeTruthy();
    }
  });

  it('has none for a rule it does not know', () => {
    expect(ruleText({ ...base, rule: 'X9' })).toBeNull();
  });
});

describe('articlesOf', () => {
  it('names each article once, in order', () => {
    expect(articlesOf([{ ...base, article: 'B 5' }, { ...base, article: 'B 1' }, { ...base, article: 'B 5' }, { ...base, article: null }]))
      .toEqual(['B 5', 'B 1']);
  });
});
