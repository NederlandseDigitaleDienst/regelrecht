import { describe, it, expect } from 'vitest';
import { articlesOf, diagnosticsOf, findingsAt, findingsByArticle, findingsText, flaggedParts, placeLabel, ruleText, type EngineDiagnostic } from './engineDiagnostics';

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

describe('findingsAt', () => {
  const onOutput: EngineDiagnostic = { ...base, place: { kind: 'output', name: 'hoogte' } };
  const onAction: EngineDiagnostic = { ...base, place: { kind: 'action', index: 2 } };
  const onInput: EngineDiagnostic = { ...base, place: { kind: 'input', name: 'inkomen' } };
  const all = [onOutput, onAction, onInput, { ...base, place: { kind: 'article' as const } }, { ...base, place: null }];

  it('puts a finding on an output on the action that assigns it', () => {
    expect(findingsAt(all, { kind: 'action', output: 'hoogte', index: 1 })).toEqual([onOutput]);
  });

  it('puts a finding on an action without an output on that position', () => {
    expect(findingsAt(all, { kind: 'action', output: null, index: 2 })).toEqual([onAction]);
    // An action with an output is never found by position.
    expect(findingsAt(all, { kind: 'action', output: 'ander', index: 2 })).toEqual([]);
  });

  it('keeps inputs and definitions with the same name apart', () => {
    expect(findingsAt(all, { kind: 'input', name: 'inkomen' })).toEqual([onInput]);
    expect(findingsAt(all, { kind: 'definition', name: 'inkomen' })).toEqual([]);
  });
});

describe('findingsText', () => {
  it('says each kind of mistake once', () => {
    expect(findingsText([{ ...base, rule: 'T1' }, { ...base, rule: 'T1' }, { ...base, rule: 'T3' }])).toBe(
      'Een rekenkundige bewerking of vergelijking krijgt iets dat geen getal is. Er worden waarden van verschillend type met elkaar vergeleken.',
    );
  });

  it('has a sentence of its own for a finding without a rule, and nothing for no findings', () => {
    expect(findingsText([{ ...base, rule: null, kind: 'load_check', message: 'the engine says' }])).toBe(
      'De engine kan dit niet lezen of uitvoeren. De technische details zeggen waarom.',
    );
    expect(findingsText([])).toBeUndefined();
  });
});

describe('findingsByArticle', () => {
  it('groups per article and leaves out a finding without one', () => {
    const groups = findingsByArticle([base, { ...base, article: '3' }, { ...base, rule: 'T1' }, { ...base, article: null }]);
    expect(groups.map((g) => [g.heading, g.findings.length])).toEqual([['Artikel 2', 2], ['Artikel 3', 1]]);
  });

  it('keeps the same article number of two other regelwerken apart, and names them', () => {
    const elsewhere = (lawId: string): EngineDiagnostic => ({ ...base, kind: 'cross_law', law_id: lawId });
    const groups = findingsByArticle([elsewhere('wet_a'), elsewhere('wet_b')]);
    expect(groups.map((g) => g.heading)).toEqual(['Wet A, artikel 2', 'Wet B, artikel 2']);
  });
});

describe('flaggedParts', () => {
  it('gives a marking under the editor\'s labels', () => {
    expect(flaggedParts({ about: 'afronden', reason: 'geen afronding', resolution: 'operation', resolved_by: 'ROUND' })).toEqual([
      { label: 'Waar het om gaat', text: 'afronden' },
      { label: 'Waarom het niet past', text: 'geen afronding' },
      { label: 'Wat ervoor nodig is', text: 'Een nieuwe bewerking in de engine: ROUND' },
    ]);
  });

  it('leaves out what an older entry does not say', () => {
    expect(flaggedParts({ about: 'afronden', reason: '', resolution: null, resolved_by: null })).toEqual([
      { label: 'Waar het om gaat', text: 'afronden' },
    ]);
    expect(flaggedParts({ about: 'x', reason: 'y', resolution: 'model', resolved_by: null })[2]).toEqual(
      { label: 'Wat ervoor nodig is', text: 'Een nieuwe vorm in het formaat' },
    );
  });
});
