import { afterEach, describe, expect, it } from 'vitest';
import { displayCell, emphasiseArguments, hasHeaderRow, isExpectation, keywordColumnWidth, splitRunResults, stepMark } from './scenarioSteps';
import { stepKeywords } from './gherkinNl';
import { adoptLocale } from '../i18n/index.js';

afterEach(() => adoptLocale('nl'));

describe('displayCell', () => {
  it('reads a stated absence as "geen", never as the literal token', () => {
    expect(displayCell('null')).toEqual({ text: 'geen', quiet: true });
    expect(displayCell(' [] ')).toEqual({ text: 'geen', quiet: true });
  });

  it('keeps "not stated" apart from "none" (RFC-036)', () => {
    expect(displayCell('   ')).toEqual({ text: 'niet opgegeven', quiet: true });
  });

  it('leaves every other value as written, without converting it', () => {
    expect(displayCell(' 209692 ')).toEqual({ text: '209692', quiet: false });
    expect(displayCell('2008-01-01')).toEqual({ text: '2008-01-01', quiet: false });
    expect(displayCell('ONBEPAALDE_TIJD')).toEqual({ text: 'ONBEPAALDE_TIJD', quiet: false });
    expect(displayCell('0')).toEqual({ text: '0', quiet: false });
  });

  it('follows the language', () => {
    adoptLocale('en');
    expect(displayCell('null').text).toBe('none');
    expect(displayCell('').text).toBe('not stated');
  });
});

describe('emphasiseArguments', () => {
  it('sets quoted arguments in bold', () => {
    expect(emphasiseArguments('is "hoogte_toeslag" gelijk aan 165412')).toBe('is **hoogte_toeslag** gelijk aan 165412');
  });

  it('keeps an empty argument visible', () => {
    expect(emphasiseArguments('parameter "x" is ""')).toBe('parameter **x** is ""');
  });
});

describe('keywordColumnWidth', () => {
  it('fits the longest keyword of the active language, in ch', () => {
    expect(keywordColumnWidth(stepKeywords())).toBe('10ch'); // Gegeven
    adoptLocale('en');
    expect(keywordColumnWidth(stepKeywords())).toBe('7ch'); // Given
    adoptLocale('fy');
    // Frisian has no keywords of its own and shows the canonical English ones.
    expect(stepKeywords()).toContain('Given');
    expect(keywordColumnWidth(stepKeywords())).toBe('7ch');
  });
});

describe('a run, split and marked the way the view shows it', () => {
  // Two background steps, then the scenario's own: a data step, the When, two Thens.
  const scenarioSteps = [
    { keyword: 'Given', text: 'the following "RvIG" data with key "bsn" for law "wet_brp":' },
    { keyword: 'When', text: 'I evaluate outputs "a, b" of "zorgtoeslagwet"' },
    { keyword: 'Then', text: 'output "a" is true' },
    { keyword: 'And', text: 'output "b" equals 1' },
  ] as const;
  // What the runner records: one result per step, background first; it stops at the failure.
  const run = [
    { status: 'pass', error: null },
    { status: 'pass', error: null },
    { status: 'pass', error: null },
    { status: 'pass', error: null },
    { status: 'pass', error: null },
    { status: 'fail', error: 'expected 1\nbut got 2' },
  ];

  it('lines the results up with the scenario steps, past the background', () => {
    const { scenario, background, backgroundError } = splitRunResults(run, 2);
    expect(background).toHaveLength(2);
    expect(backgroundError).toBeNull();
    expect(scenario).toHaveLength(scenarioSteps.length);
    expect(scenarioSteps.map((step, i) => stepMark(step, scenario[i])?.color ?? null)).toEqual([null, null, 'success', 'critical']);
    expect(scenario[3]?.error).toBe('expected 1\nbut got 2');
  });

  it('reports a failed background step apart, with no scenario results after it', () => {
    const { scenario, backgroundError } = splitRunResults([{ status: 'pass' }, { status: 'fail', error: 'geen peildatum' }], 2);
    expect(backgroundError).toBe('geen peildatum');
    expect(scenario).toEqual([]);
  });

  it('has nothing before a run', () => {
    expect(splitRunResults(undefined, 2)).toEqual({ background: [], scenario: [], backgroundError: null });
  });

  it('marks a failed Given or When too, but a pass only on an expectation', () => {
    expect(stepMark(scenarioSteps[0], { status: 'fail' })?.icon).toBe('dismiss-circle');
    expect(stepMark(scenarioSteps[1], { status: 'fail' })?.icon).toBe('dismiss-circle');
    expect(stepMark(scenarioSteps[1], { status: 'pass' })).toBeNull();
    expect(stepMark(scenarioSteps[2], { status: 'pass' })?.icon).toBe('check-mark-circle');
    expect(stepMark(scenarioSteps[2], null)).toBeNull();
  });
});

describe('hasHeaderRow', () => {
  it('is false only for the parameters table', () => {
    expect(hasHeaderRow({ text: 'the following parameters:' })).toBe(false);
    expect(hasHeaderRow({ text: 'the following "RvIG" data with key "bsn" for law "wet_brp":' })).toBe(true);
    expect(hasHeaderRow({ text: 'parameter "kinderen" is the collection:' })).toBe(true);
  });
});

describe('isExpectation', () => {
  it('knows a Then step by the grammar, not by its keyword', () => {
    expect(isExpectation({ keyword: 'And', text: 'output "hoogte_toeslag" equals 1' })).toBe(true);
    expect(isExpectation({ keyword: 'Then', text: 'the execution succeeds' })).toBe(true);
    expect(isExpectation({ keyword: 'And', text: 'parameter "bsn" is "1"' })).toBe(false);
    expect(isExpectation({ keyword: 'Then', text: 'not a step' })).toBe(false);
  });
});
