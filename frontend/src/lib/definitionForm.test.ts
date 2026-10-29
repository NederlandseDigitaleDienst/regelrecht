import { describe, it, expect } from 'vitest';
import {
  changeType,
  formatDefinitionValue,
  fromForm,
  newDefinitionForm,
  toForm,
  valueSuffix,
} from './definitionForm';
import type { RawDefinition } from './lawFormat';

/** Open the form and save it untouched. */
function roundTrip(raw: RawDefinition) {
  return fromForm(toForm(raw), raw);
}

describe('definitionForm', () => {
  describe('an untouched definition saves as it was', () => {
    const shapes: [string, RawDefinition][] = [
      ['a bare number', 0.04273],
      ['a bare text', 'Minister van Volksgezondheid, Welzijn en Sport'],
      ['a bare boolean', true],
      ['a bare list', ['A', 'B']],
      ['a structured value without a type', { value: 5587500 }],
      ['a list without a type', { value: ['HUWELIJK', 'GEREGISTREERD_PARTNERSCHAP'] }],
      ['an amount in eurocent', { value: 3971900, type: 'amount', type_spec: { unit: 'eurocent' } }],
      ['a ratio', { value: 0.01896, type: 'number', type_spec: { unit: 'ratio' } }],
      ['a percentage', { value: 13.7, type: 'number', type_spec: { unit: 'percentage' } }],
      ['a duration', { value: 4, type: 'number', type_spec: { unit: 'years' } }],
      ['a number without a unit', { value: 3, type: 'number' }],
      ['a text', { value: 'Dienst Toeslagen', type: 'string' }],
      ['a boolean', { value: false, type: 'boolean' }],
      ['a date', { value: '2006-01-01', type: 'date' }],
      ['a list of numbers', { value: [1, 2, 3], type: 'array' }],
    ];
    for (const [label, raw] of shapes) {
      it(label, () => expect(roundTrip(raw)).toEqual(raw));
    }
  });

  it('keeps keys it does not edit', () => {
    const raw = {
      value: 0.137,
      type: 'number',
      description: 'percentage van het toetsingsinkomen',
      type_spec: { unit: 'ratio', precision: 4 },
    };
    const form = toForm(raw);
    form.number = 0.138;
    expect(fromForm(form, raw)).toEqual({ ...raw, value: 0.138 });
  });

  it('shows an amount in euros and stores it in eurocent', () => {
    const raw = { value: 3971900, type: 'amount', type_spec: { unit: 'eurocent' } };
    const form = toForm(raw);
    expect(form.number).toBe(39719);
    form.number = 39719.5;
    expect(fromForm(form, raw)).toEqual({ ...raw, value: 3971950 });
  });

  it('never converts a percentage or a ratio', () => {
    const pct = toForm({ value: 13.7, type: 'number', type_spec: { unit: 'percentage' } });
    expect(pct.number).toBe(13.7);
    const ratio = toForm({ value: 0.137, type: 'number', type_spec: { unit: 'ratio' } });
    expect(ratio.number).toBe(0.137);
    // An untyped number between 0 and 1 is no longer read as a percentage.
    expect(toForm(0.04273).number).toBe(0.04273);
  });

  describe('a bare value', () => {
    it('stays bare while it has no type', () => {
      const form = toForm(0.04273);
      form.number = 0.05;
      expect(fromForm(form, 0.04273)).toBe(0.05);
    });

    it('becomes structured once the user gives it a type and unit', () => {
      const form = toForm(13.7);
      changeType(form, 'number');
      form.unit = 'percentage';
      expect(fromForm(form, 13.7)).toEqual({ value: 13.7, type: 'number', type_spec: { unit: 'percentage' } });
    });
  });

  describe('changing the type', () => {
    it('writes eurocent for an amount', () => {
      const form = toForm({ value: 100 });
      changeType(form, 'amount');
      form.number = 12.34;
      expect(fromForm(form, { value: 100 })).toEqual({ value: 1234, type: 'amount', type_spec: { unit: 'eurocent' } });
    });

    it('drops a number unit when the value stops being a number', () => {
      const raw = { value: 4, type: 'number', type_spec: { unit: 'years' } };
      const form = toForm(raw);
      changeType(form, 'string');
      expect(fromForm(form, raw)).toEqual({ value: '4', type: 'string' });
    });

    it('carries the value across kinds', () => {
      const form = toForm({ value: 'true', type: 'string' });
      changeType(form, 'boolean');
      expect(form.bool).toBe(true);
      changeType(form, 'array');
      expect(form.items).toEqual(['true']);
      changeType(form, 'number');
      expect(form.number).toBe(0);
    });

    it('keeps a valid ISO date and drops anything else', () => {
      const ok = toForm({ value: '2025-01-01', type: 'string' });
      changeType(ok, 'date');
      expect(ok.date).toBe('2025-01-01');
      const not = toForm({ value: 'volgend jaar', type: 'string' });
      changeType(not, 'date');
      expect(not.date).toBe('');
    });

    it('removes the type again when set back to none', () => {
      const raw = { value: 3, type: 'number' };
      const form = toForm(raw);
      changeType(form, '');
      expect(fromForm(form, raw)).toEqual({ value: 3 });
    });
  });

  describe('the stored number never changes with the type', () => {
    it('keeps eurocent when a bare number becomes an amount', () => {
      const form = toForm(3971900);
      changeType(form, 'amount');
      expect(form.number).toBe(39719);
      expect(fromForm(form, 3971900)).toEqual({ value: 3971900, type: 'amount', type_spec: { unit: 'eurocent' } });
    });

    it('keeps eurocent when an amount becomes a number', () => {
      const raw = { value: 3971900, type: 'amount', type_spec: { unit: 'eurocent' } };
      const form = toForm(raw);
      changeType(form, 'number');
      expect(form.number).toBe(3971900);
      expect(fromForm(form, raw)).toEqual({ value: 3971900, type: 'number' });
    });

    it('keeps eurocent, and drops the unit, when the type is cleared', () => {
      const raw = { value: 3971900, type: 'amount', type_spec: { unit: 'eurocent' } };
      const form = toForm(raw);
      changeType(form, '');
      expect(form.number).toBe(3971900);
      expect(fromForm(form, raw)).toEqual({ value: 3971900 });
    });

    it('keeps a unit the form never showed on a definition that never had a type', () => {
      const raw = { value: 150000, type_spec: { unit: 'eurocent' } };
      const form = toForm(raw);
      form.number = 160000;
      expect(fromForm(form, raw)).toEqual({ value: 160000, type_spec: { unit: 'eurocent' } });
    });
  });

  describe('a shape the form would have to guess at is kept as it is', () => {
    const kept: [string, RawDefinition, string][] = [
      ['a map without value', { algemene_volmacht: 'x', bijzondere_volmacht: 'y' }, 'samengestelde definitie zonder value'],
      ['a null value', { value: null, type: 'number' }, 'lege waarde'],
      ['a number in eurocent', { value: 5, type: 'number', type_spec: { unit: 'eurocent' } }, 'getal in eurocent'],
      ['an amount without a unit', { value: 5, type: 'amount' }, 'bedrag zonder eenheid'],
      ['a boolean stored as text', { value: 'true', type: 'boolean' }, 'waarde die niet past bij type boolean'],
      ['a number stored as text', { value: 'abc', type: 'number' }, 'waarde die niet past bij type number'],
      ['a text stored as a number', { value: 5, type: 'string' }, 'waarde die niet past bij type string'],
      ['a list of mixed values', { value: [1, 'a'] }, 'lijst met gemengde waarden'],
      ['a list of maps', [{ a: 1 }], 'lijst met gemengde waarden'],
      ['a map as value', { value: { a: 1 } }, 'samengestelde waarde'],
    ];
    for (const [label, raw, reason] of kept) {
      it(label, () => {
        const form = toForm(raw);
        expect(form.unsupported).toBe(reason);
        expect(fromForm(form, raw)).toBe(raw);
      });
    }
  });

  it('starts a new definition as a structured number', () => {
    const form = newDefinitionForm();
    form.number = 42;
    expect(fromForm(form, null)).toEqual({ value: 42, type: 'number' });
  });

  it('leaves a type it does not edit alone', () => {
    const raw = { value: { a: 1 }, type: 'object' };
    const form = toForm(raw);
    expect(form.unsupported).toBe('type object');
    expect(fromForm(form, raw)).toBe(raw);
    const euro = { value: 12.5, type: 'amount', type_spec: { unit: 'euro' } };
    expect(toForm(euro).unsupported).toBe('bedrag in euro');
    expect(fromForm(toForm(euro), euro)).toBe(euro);
  });

  it('turns list items that were numbers back into numbers', () => {
    const raw = { value: [1, 2], type: 'array' };
    const form = toForm(raw);
    form.items = ['1', '2', '3'];
    expect(fromForm(form, raw)).toEqual({ value: [1, 2, 3], type: 'array' });
  });

  it('says what a ratio amounts to, and names the other units', () => {
    expect(valueSuffix(toForm({ value: 0.137, type: 'number', type_spec: { unit: 'ratio' } }))).toBe('= 13,7%');
    expect(valueSuffix(toForm({ value: 13.7, type: 'number', type_spec: { unit: 'percentage' } }))).toBe('%');
    expect(valueSuffix(toForm({ value: 4, type: 'number', type_spec: { unit: 'years' } }))).toBe('jaar');
    expect(valueSuffix(toForm(0.137))).toBe('');
  });

  it('formats a definition for reading by its unit, and nothing else', () => {
    // Intl puts a non-breaking space between the sign and the amount.
    expect(formatDefinitionValue(3971900, 'eurocent')).toBe('€\u00a039.719,00');
    expect(formatDefinitionValue(13.7, 'percentage')).toBe('13,7%');
    expect(formatDefinitionValue(0.01896, 'ratio')).toBe('0,01896 (1,896%)');
    expect(formatDefinitionValue(4, 'years')).toBe('4 jaar');
    expect(formatDefinitionValue(0.04273, undefined)).toBe('0.04273');
    expect(formatDefinitionValue(['A', 'B'], undefined)).toBe('A, B');
    expect(formatDefinitionValue(null, undefined)).toBe('');
  });
});
