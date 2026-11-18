import { describe, expect, it } from 'vitest';
import { singleParam, singleQuery } from './routeParams';

describe('singleParam', () => {
  it('passes a string and undefined through unchanged', () => {
    expect(singleParam('wet_op_de_zorgtoeslag')).toBe('wet_op_de_zorgtoeslag');
    expect(singleParam('')).toBe('');
    expect(singleParam(undefined)).toBe(undefined);
  });

  it('takes the first element of an array (only a repeatable segment yields one)', () => {
    expect(singleParam(['a', 'b'])).toBe('a');
    expect(singleParam([])).toBe(undefined);
  });
});

describe('singleQuery', () => {
  it('passes a string through and maps a missing value to undefined', () => {
    expect(singleQuery('wet_op_de_zorgtoeslag')).toBe('wet_op_de_zorgtoeslag');
    expect(singleQuery('')).toBe('');
    expect(singleQuery(undefined)).toBe(undefined);
  });

  it('maps a valueless key (`?law`, which vue-router gives as null) to undefined', () => {
    expect(singleQuery(null)).toBe(undefined);
  });

  it('takes the first value of a repeated key (`?law=a&law=b`)', () => {
    expect(singleQuery(['a', 'b'])).toBe('a');
    expect(singleQuery([null, 'b'])).toBe(undefined);
    expect(singleQuery([])).toBe(undefined);
  });
});
