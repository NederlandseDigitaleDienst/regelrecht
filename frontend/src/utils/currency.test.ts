import { describe, it, expect } from 'vitest';
import { centsToEuros, eurosToCents } from './currency';

describe('currency', () => {
  describe('centsToEuros', () => {
    it('converts eurocents to a 2-decimal euro number', () => {
      expect(centsToEuros(150000)).toBe(1500);
      expect(centsToEuros(109171)).toBe(1091.71);
      expect(centsToEuros(1)).toBe(0.01);
    });

    it('passes empty/nullish through unchanged', () => {
      expect(centsToEuros('')).toBe('');
      expect(centsToEuros(null)).toBe('');
      expect(centsToEuros(undefined)).toBe('');
    });

    it('accepts numeric strings', () => {
      expect(centsToEuros('109171')).toBe(1091.71);
    });
  });

  describe('eurosToCents', () => {
    it('converts euros to rounded eurocents', () => {
      expect(eurosToCents(1500)).toBe(150000);
      expect(eurosToCents(1091.71)).toBe(109171);
      expect(eurosToCents(19.999)).toBe(2000);
    });

    it('passes empty/nullish through unchanged', () => {
      expect(eurosToCents('')).toBe('');
      expect(eurosToCents(null)).toBe('');
      expect(eurosToCents(undefined)).toBe('');
    });

    it('round-trips with centsToEuros', () => {
      expect(eurosToCents(centsToEuros(109171))).toBe(109171);
    });
  });
});
