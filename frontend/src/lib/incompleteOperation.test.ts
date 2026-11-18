import { describe, expect, it } from 'vitest';
import { findIncompleteOperation } from './incompleteOperation';

// The stubs below are the ones OperationSettings seeds for a new or retyped
// operation; each must be refused until it is filled in.
describe('findIncompleteOperation', () => {
  it('accepts literals and complete operations', () => {
    expect(findIncompleteOperation(5)).toBeNull();
    expect(findIncompleteOperation('$naam')).toBeNull();
    expect(findIncompleteOperation({ operation: 'ADD', values: ['$a', 1] })).toBeNull();
    expect(findIncompleteOperation({ operation: 'NOT_NULL', subject: '$a' })).toBeNull();
    expect(findIncompleteOperation({ operation: 'IN', subject: '$a', value: [1, 2] })).toBeNull();
  });

  it.each([
    ['an arithmetic op without values', { operation: 'ADD', values: [] }, 'ADD'],
    ['a logical op without conditions', { operation: 'AND', conditions: [] }, 'AND'],
    ['an IF without cases', { operation: 'IF', cases: [] }, 'IF'],
    ['a comparison without subject', { operation: 'EQUALS', subject: '', value: 1 }, 'EQUALS'],
    ['a comparison without value', { operation: 'GREATER_THAN', subject: '$a', value: '' }, 'GREATER_THAN'],
    ['an IN with an empty list', { operation: 'IN', subject: '$a', value: [] }, 'IN'],
    ['a NOT without value', { operation: 'NOT', value: '' }, 'NOT'],
    ['an AGE without a reference date', { operation: 'AGE', date_of_birth: '$geb', reference_date: '' }, 'AGE'],
  ])('refuses %s', (_label, tree, op) => {
    expect(findIncompleteOperation(tree)).toBe(op);
  });

  it('finds an incomplete operation nested anywhere in the tree', () => {
    const tree = {
      operation: 'IF',
      cases: [{ when: { operation: 'EQUALS', subject: '$a', value: 1 }, then: { operation: 'MULTIPLY', values: [] } }],
    };
    expect(findIncompleteOperation(tree)).toBe('MULTIPLY');
    expect(findIncompleteOperation({ operation: 'ADD', values: [1, { operation: 'OR', conditions: [] }] })).toBe('OR');
  });
});
