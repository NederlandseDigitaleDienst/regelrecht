import { describe, it, expect } from 'vitest';
import { quoteContext } from './quoteContext';

// Helper: locate a fragment by substring and run quoteContext on it.
function ctx(text: string, fragment: string) {
  const start = text.indexOf(fragment);
  return quoteContext(text, start, start + fragment.length);
}

describe('quoteContext', () => {
  it('takes up to 3 words each side and adds ellipses when truncated', () => {
    const text = 'een twee drie vier VIJF zes zeven acht negen tien elf twaalf';
    // Fragment "VIJF" is lowercase-surrounded on both sides, far from any
    // sentence boundary, so both sides truncate at 3 words with an ellipsis.
    const r = ctx(text, 'VIJF');
    expect(r.before).toBe('twee drie vier ');
    expect(r.quote).toBe('VIJF');
    expect(r.after).toBe(' zes zeven acht');
    expect(r.ellipsisBefore).toBe('… ');
    expect(r.ellipsisAfter).toBe(' …');
  });

  it('stops the left context at a capitalised word and drops the left ellipsis', () => {
    const text = 'Indien de normpremie voor een verzekerde hoger is';
    const r = ctx(text, 'normpremie');
    expect(r.before).toBe('Indien de ');
    expect(r.ellipsisBefore).toBe(''); // began at the sentence start
  });

  it('stops the right context at a period and drops the right ellipsis', () => {
    const text = 'geacht gezamenlijk een aanspraak te hebben. De rest volgt hierna';
    const r = ctx(text, 'aanspraak');
    expect(r.after).toBe(' te hebben.');
    expect(r.ellipsisAfter).toBe(''); // ended at the sentence end
  });

  it('shows no before context or ellipsis at the start of the text', () => {
    const text = 'normpremie voor een verzekerde';
    const r = ctx(text, 'normpremie');
    expect(r.before).toBe('');
    expect(r.ellipsisBefore).toBe('');
  });

  it('shows no after context or ellipsis at the end of the text', () => {
    const text = 'de hoogte van de normpremie';
    const r = ctx(text, 'normpremie');
    expect(r.after).toBe('');
    expect(r.ellipsisAfter).toBe('');
  });

  it('caps the left context at 3 words even without a capital', () => {
    const text = 'aa bb cc dd ee ff normpremie xx';
    const r = ctx(text, 'normpremie');
    expect(r.before).toBe('dd ee ff ');
    expect(r.ellipsisBefore).toBe('… ');
  });

  it('keeps only the period-terminated word on the right, not the next sentence', () => {
    const text = 'de normpremie hoger. Een nieuwe zin begint hier';
    const r = ctx(text, 'normpremie');
    expect(r.after).toBe(' hoger.');
    expect(r.ellipsisAfter).toBe('');
  });
});
