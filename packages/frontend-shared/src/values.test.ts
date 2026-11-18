import { describe, it, expect } from 'vitest';
import { isUnknown, missingFacts } from './values';

const unknown = {
  __unknown: true,
  missing: [{ law: 'test_wet', name: 'huur', kind: 'no_data' }],
};

describe('isUnknown', () => {
  it('recognizes the engine shape and nothing else', () => {
    expect(isUnknown(unknown)).toBe(true);
    expect(isUnknown({ __unknown: true, missing: [] })).toBe(true);
    expect(isUnknown(null)).toBe(false);
    expect(isUnknown(undefined)).toBe(false);
    expect(isUnknown({})).toBe(false);
    expect(isUnknown({ __unknown: false, missing: [] })).toBe(false);
    expect(isUnknown({ __unknown: 'true' })).toBe(false);
    expect(isUnknown([unknown])).toBe(false);
    expect(isUnknown('__unknown')).toBe(false);
    expect(isUnknown(0)).toBe(false);
  });
});

describe('missingFacts', () => {
  it('returns the facts of an unknown value', () => {
    expect(missingFacts(unknown)).toEqual([{ law: 'test_wet', name: 'huur', kind: 'no_data' }]);
  });

  it('is empty for any other value, without a guard', () => {
    expect(missingFacts(null)).toEqual([]);
    expect(missingFacts(undefined)).toEqual([]);
    expect(missingFacts(42)).toEqual([]);
    expect(missingFacts({ missing: [{ name: 'x' }] })).toEqual([]);
    expect(missingFacts({ __unknown: true })).toEqual([]);
  });
});
