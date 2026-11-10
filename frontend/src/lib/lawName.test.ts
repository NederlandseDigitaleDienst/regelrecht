import { describe, it, expect } from 'vitest';
import { humanizeLawId, resolveLawName } from './lawName';
import type { LawDocument } from './lawFormat';

describe('humanizeLawId', () => {
  it('turns a snake_case law id into title-cased words', () => {
    expect(humanizeLawId('algemene_wet_inkomensafhankelijke_regelingen')).toBe(
      'Algemene Wet Inkomensafhankelijke Regelingen',
    );
  });

  it('keeps digits and capitalises each word', () => {
    expect(humanizeLawId('burgerlijk_wetboek_boek_5')).toBe('Burgerlijk Wetboek Boek 5');
  });

  it('returns an empty string for nullish/empty input', () => {
    expect(humanizeLawId(null)).toBe('');
    expect(humanizeLawId(undefined)).toBe('');
    expect(humanizeLawId('')).toBe('');
  });
});

describe('resolveLawName', () => {
  it('returns the action value a #name reference points at', () => {
    const law = {
      $id: 'wet_x',
      name: '#naam',
      articles: [{ number: '1', machine_readable: { execution: { actions: [{ output: 'naam', value: 'Wet X' }] } } }],
    } as unknown as LawDocument;
    expect(resolveLawName(law)).toBe('Wet X');
  });

  it('never returns a non-string action value as the name', () => {
    const law = {
      $id: 'wet_x',
      name: '#naam',
      articles: [
        { number: '1', machine_readable: { execution: { actions: [{ output: 'naam', value: { operation: 'ADD' } }] } } },
      ],
    } as unknown as LawDocument;
    expect(typeof resolveLawName(law)).toBe('string');
    expect(resolveLawName(law)).toBe('#naam');
  });
});
