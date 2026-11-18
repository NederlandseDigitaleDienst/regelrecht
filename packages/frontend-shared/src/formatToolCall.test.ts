import { describe, it, expect } from 'vitest';
import { formatToolCall, formatToolValue } from './formatToolCall';

describe('formatToolCall', () => {
  it('shows the options of a question by their label, not as [object Object]', () => {
    const line = formatToolCall('vraag_beleidsmaker', {
      vraag: 'Welk regime pas je aan?',
      opties: [
        { label: 'SF15-oud', gevolg: 'raakt bestaande debiteuren' },
        { label: 'SF35', gevolg: 'alleen nieuwe' },
      ],
    });
    expect(line).toBe('vraag_beleidsmaker · vraag: Welk regime pas je aan? · opties: SF15-oud / SF35');
    expect(line).not.toContain('[object Object]');
  });

  it('separates the tool name from its first argument', () => {
    expect(formatToolCall('simuleer_populatie', { n: 42 })).toBe('simuleer_populatie · n: 42');
  });

  it('is just the name without arguments', () => {
    expect(formatToolCall('lees_corpus', undefined)).toBe('lees_corpus');
    expect(formatToolCall('lees_corpus', {})).toBe('lees_corpus');
  });

  it('leaves out empty arguments', () => {
    expect(formatToolCall('t', { a: null, b: '', c: 1 })).toBe('t · c: 1');
  });
});

describe('formatToolValue', () => {
  it('counts a list of objects without a recognisable name', () => {
    expect(formatToolValue([{ x: 1 }, { y: 2 }])).toBe('2 items');
  });

  it('joins a list of plain values', () => {
    expect(formatToolValue(['a', 2, true])).toBe('a / 2 / true');
  });

  it('names an object by its label, else shows it as JSON', () => {
    expect(formatToolValue({ naam: 'variant A', extra: 1 })).toBe('variant A');
    expect(formatToolValue({ a: 1 })).toBe('{"a":1}');
  });

  it('shortens long text and flattens whitespace', () => {
    const text = formatToolValue(`regel een\n\nregel twee ${'x'.repeat(200)}`);
    expect(text.startsWith('regel een regel twee')).toBe(true);
    expect(text.length).toBe(120);
    expect(text.endsWith('…')).toBe(true);
  });
});
