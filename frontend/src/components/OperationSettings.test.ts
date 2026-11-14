import { describe, it, expect, beforeAll } from 'vitest';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import OperationSettings from './OperationSettings.vue';
import type { OperationNode, OperationTreeRow } from '../utils/operationTree';

/** A test node: an operation type plus whatever slots the test sets or reads. */
type TestNode = { operation: string; [key: string]: unknown };

/** The value-row fields the tests pass in; the component fills the rest. */
type TestRow = { _kind: string; _label?: string; _value?: unknown; _index?: number; _caseIndex?: number };

/** The script-setup bindings the tests drive through wrapper.vm. */
interface OperationSettingsVm {
  operationValues: TestRow[];
  canAddValue: boolean;
  changeOperationType(newType: string): void;
  applyValueMutation(row: TestRow, newVal: unknown): void;
  canRemoveValue(row: TestRow): boolean;
  canChangeValueKind(row: TestRow): boolean;
  typeOptions: { value: string; label: string }[];
}

// `nldd-*` tags are configured as custom elements in vite.config.js, so
// Vue Test Utils' stub system doesn't replace them. None of the tests
// here interact with the rendered NLDD elements (we drive the component
// via wrapper.vm), so we leave them as raw HTMLElement and only stub the
// methods that the watcher / lifecycle would otherwise complain about.
beforeAll(() => {
  // No custom element registration needed for OperationSettings - it
  // doesn't call show()/hide() on any host the way EditSheet does.
});

/**
 * The component takes an `operation` prop in the shape produced by
 * buildOperationTree(): `{ number, title, subtitle, operation, values, node }`.
 * For these tests we only care about `operation` (the type) and `node` (the
 * mutable underlying object that operationValues / mutations read and
 * write). Helper to build a minimal valid prop.
 */
function makeOpProp(node: TestNode): OperationTreeRow {
  return {
    number: '1',
    title: 'test op',
    subtitle: '',
    operation: node.operation,
    values: (node.values as unknown[] | undefined) || [],
    // A test node is a loosely typed operation node.
    node: node as OperationNode,
  };
}

function mountOp(node: TestNode, { editable = true } = {}) {
  const wrapper = mount(OperationSettings, {
    props: {
      operation: makeOpProp(node),
      article: { machine_readable: { execution: { input: [] } } },
      editable,
    },
  });
  // Script-setup bindings are reachable on vm at runtime, but not in its type.
  return wrapper as unknown as Omit<VueWrapper, 'vm'> & { vm: OperationSettingsVm };
}

describe('OperationSettings - AGE op', () => {
  describe('operationValues', () => {
    it('returns date_of_birth and reference_date rows for an AGE node', () => {
      const node: TestNode = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);
      const rows = wrapper.vm.operationValues;
      expect(rows).toHaveLength(2);
      expect(rows[0]).toMatchObject({
        _label: 'Geboortedatum',
        _value: '$geboortedatum',
        _kind: 'date_of_birth',
      });
      expect(rows[1]).toMatchObject({
        _label: 'Peildatum',
        _value: '2025-01-01',
        _kind: 'reference_date',
      });
    });

    it('falls back to empty strings for missing AGE fields', () => {
      const node: TestNode = { operation: 'AGE' };
      const wrapper = mountOp(node);
      const rows = wrapper.vm.operationValues;
      expect(rows[0]!._value).toBe('');
      expect(rows[1]!._value).toBe('');
    });
  });

  describe('changeOperationType to AGE', () => {
    it('seeds date_of_birth and reference_date as empty strings', async () => {
      const node: TestNode = {
        operation: 'EQUALS',
        subject: '$foo',
        value: 42,
      };
      const wrapper = mountOp(node);

      // Simulate the dropdown change. The handler reads event.target.value.
      wrapper.vm.changeOperationType('AGE');
      await nextTick();

      expect(node.operation).toBe('AGE');
      expect(node.date_of_birth).toBe('');
      expect(node.reference_date).toBe('');
      // The old comparison-shape fields are stripped.
      expect(node.subject).toBeUndefined();
      expect(node.value).toBeUndefined();
    });

    it('preserves existing AGE fields when re-entering AGE', async () => {
      const node: TestNode = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);

      // Switching to the same op type is a no-op (early return).
      wrapper.vm.changeOperationType('AGE');
      await nextTick();

      expect(node.date_of_birth).toBe('$geboortedatum');
      expect(node.reference_date).toBe('2025-01-01');
    });

    it('strips AGE fields when switching back to a comparison op', async () => {
      const node: TestNode = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);

      wrapper.vm.changeOperationType('EQUALS');
      await nextTick();

      expect(node.operation).toBe('EQUALS');
      expect(node.subject).toBe('');
      expect(node.value).toBe('');
      // The schema sets `additionalProperties: false` on every operation
      // type, so any leaked AGE field would fail validation on save.
      // Both must be removed by the EQUALS branch.
      expect(node.date_of_birth).toBeUndefined();
      expect(node.reference_date).toBeUndefined();
    });

    it.each([
      ['AND', { conditions: [] }],
      ['IF', { cases: expect.any(Array), default: 0 }],
      ['NOT', { value: '' }],
      ['SWITCH', { cases: expect.any(Array), default: 0 }],
      ['ADD', { values: [] }],
    ])('strips AGE fields when switching to %s', async (newType, _expected) => {
      const node: TestNode = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);

      wrapper.vm.changeOperationType(newType);
      await nextTick();

      expect(node.operation).toBe(newType);
      expect(node.date_of_birth).toBeUndefined();
      expect(node.reference_date).toBeUndefined();
    });
  });

  describe('applyValueMutation', () => {
    it('writes date_of_birth on the node', () => {
      const node: TestNode = { operation: 'AGE', date_of_birth: '', reference_date: '' };
      const wrapper = mountOp(node);
      wrapper.vm.applyValueMutation({ _kind: 'date_of_birth' }, '$geboortedatum');
      expect(node.date_of_birth).toBe('$geboortedatum');
    });

    it('writes reference_date on the node', () => {
      const node: TestNode = { operation: 'AGE', date_of_birth: '', reference_date: '' };
      const wrapper = mountOp(node);
      wrapper.vm.applyValueMutation({ _kind: 'reference_date' }, '2025-01-01');
      expect(node.reference_date).toBe('2025-01-01');
    });
  });

  describe('canRemoveValue', () => {
    it('blocks removal of both AGE structural fields', () => {
      const node: TestNode = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);
      expect(wrapper.vm.canRemoveValue({ _kind: 'date_of_birth' })).toBe(false);
      expect(wrapper.vm.canRemoveValue({ _kind: 'reference_date' })).toBe(false);
    });
  });

  describe('canAddValue', () => {
    // Operations are added by switching a value's Type via the per-row
    // menu, so there is only the single "Voeg waarde toe" path now.
    it('disables the add-value button for AGE', () => {
      const node: TestNode = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);
      expect(wrapper.vm.canAddValue).toBe(false);
    });
  });
});

describe('OperationSettings - DATE_DIFF op', () => {
  function dateDiffNode(): TestNode {
    return {
      operation: 'DATE_DIFF',
      from: '$indieningsdatum',
      to: '$referencedate.iso',
      in: 'days',
    };
  }

  describe('operationValues', () => {
    it('returns from, to and in rows for a DATE_DIFF node', () => {
      const wrapper = mountOp(dateDiffNode());
      const rows = wrapper.vm.operationValues;
      expect(rows).toHaveLength(3);
      expect(rows[0]).toMatchObject({ _label: 'Van', _value: '$indieningsdatum', _kind: 'from' });
      expect(rows[1]).toMatchObject({ _label: 'Tot', _value: '$referencedate.iso', _kind: 'to' });
      expect(rows[2]).toMatchObject({ _label: 'Eenheid', _value: 'days', _kind: 'in' });
    });

    it('falls back to empty strings for missing DATE_DIFF fields', () => {
      const wrapper = mountOp({ operation: 'DATE_DIFF' });
      const rows = wrapper.vm.operationValues;
      expect(rows.map((r) => r._value)).toEqual(['', '', '']);
    });
  });

  describe('changeOperationType to DATE_DIFF', () => {
    it('seeds from/to as empty strings and in with a valid unit', async () => {
      const node: TestNode = { operation: 'EQUALS', subject: '$foo', value: 42 };
      const wrapper = mountOp(node);

      wrapper.vm.changeOperationType('DATE_DIFF');
      await nextTick();

      expect(node.operation).toBe('DATE_DIFF');
      expect(node.from).toBe('');
      expect(node.to).toBe('');
      // The schema requires `in` to be one of the unit enum values, so the
      // seed must already validate.
      expect(node.in).toBe('days');
      expect(node.subject).toBeUndefined();
      expect(node.value).toBeUndefined();
    });

    it.each([
      ['EQUALS'],
      ['AND'],
      ['IF'],
      ['NOT'],
      ['SWITCH'],
      ['ADD'],
      ['AGE'],
    ])('strips DATE_DIFF fields when switching to %s', async (newType) => {
      const node = dateDiffNode();
      const wrapper = mountOp(node);

      wrapper.vm.changeOperationType(newType);
      await nextTick();

      expect(node.operation).toBe(newType);
      // `additionalProperties: false` on every operation type means a leaked
      // from/to/in field would fail validation on save.
      expect(node.from).toBeUndefined();
      expect(node.to).toBeUndefined();
      expect(node.in).toBeUndefined();
    });
  });

  describe('applyValueMutation', () => {
    it('writes from, to and in on the node', () => {
      const node: TestNode = { operation: 'DATE_DIFF', from: '', to: '', in: 'days' };
      const wrapper = mountOp(node);
      wrapper.vm.applyValueMutation({ _kind: 'from' }, '$indieningsdatum');
      wrapper.vm.applyValueMutation({ _kind: 'to' }, '$referencedate.iso');
      wrapper.vm.applyValueMutation({ _kind: 'in' }, 'months');
      expect(node.from).toBe('$indieningsdatum');
      expect(node.to).toBe('$referencedate.iso');
      expect(node.in).toBe('months');
    });
  });

  describe('canRemoveValue', () => {
    it('blocks removal of all three DATE_DIFF structural fields', () => {
      const wrapper = mountOp(dateDiffNode());
      expect(wrapper.vm.canRemoveValue({ _kind: 'from' })).toBe(false);
      expect(wrapper.vm.canRemoveValue({ _kind: 'to' })).toBe(false);
      expect(wrapper.vm.canRemoveValue({ _kind: 'in' })).toBe(false);
    });
  });

  describe('canAddValue', () => {
    it('disables the add-value button for DATE_DIFF', () => {
      const wrapper = mountOp(dateDiffNode());
      expect(wrapper.vm.canAddValue).toBe(false);
    });
  });

  describe('canChangeValueKind', () => {
    it('allows from/to to become nested operations but keeps in literal-only', () => {
      const wrapper = mountOp(dateDiffNode());
      expect(wrapper.vm.canChangeValueKind({ _kind: 'from' })).toBe(true);
      expect(wrapper.vm.canChangeValueKind({ _kind: 'to' })).toBe(true);
      expect(wrapper.vm.canChangeValueKind({ _kind: 'in' })).toBe(false);
    });
  });
});

describe('OperationSettings - FOREACH op', () => {
  function foreachNode(): TestNode {
    return {
      operation: 'FOREACH',
      collection: '$medebewoners',
      as: 'medebewoner',
      body: '$medebewoner.bijdrage',
      combine: 'ADD',
    };
  }

  describe('operationValues', () => {
    it('returns the collection, binding, body and combine rows', () => {
      const wrapper = mountOp(foreachNode());
      const rows = wrapper.vm.operationValues;
      expect(rows).toHaveLength(4);
      expect(rows[0]).toMatchObject({ _label: 'Verzameling', _value: '$medebewoners', _kind: 'collection' });
      expect(rows[1]).toMatchObject({ _label: 'Naam per element', _value: 'medebewoner', _kind: 'as' });
      expect(rows[2]).toMatchObject({ _label: 'Per element', _value: '$medebewoner.bijdrage', _kind: 'body' });
      expect(rows[3]).toMatchObject({ _label: 'Samenvoegen', _value: 'ADD', _kind: 'combine' });
    });

    it('shows the filter row between the binding and the body when present', () => {
      const node: TestNode = { ...foreachNode(), filter: { operation: 'GREATER_THAN', subject: '$x', value: 21 } };
      const rows = mountOp(node).vm.operationValues;
      expect(rows.map((r) => r._kind)).toEqual(['collection', 'as', 'filter', 'body', 'combine']);
    });

    it('omits the optional filter and combine rows when absent', () => {
      const rows = mountOp({ operation: 'FOREACH', collection: '$x', as: 'i', body: '$i' }).vm.operationValues;
      expect(rows.map((r) => r._kind)).toEqual(['collection', 'as', 'body']);
    });
  });

  describe('changeOperationType to FOREACH', () => {
    it('seeds an empty collection and body, and the schema default binding', () => {
      const node: TestNode = { operation: 'EQUALS', subject: '$foo', value: 42 };
      const wrapper = mountOp(node);

      wrapper.vm.changeOperationType('FOREACH');

      expect(node.operation).toBe('FOREACH');
      expect(node.collection).toBe('');
      expect(node.body).toBe('');
      // 'item' is the schema default, so the seeded node already validates.
      expect(node.as).toBe('item');
      // filter and combine are optional and stay absent until asked for.
      expect(node.filter).toBeUndefined();
      expect(node.combine).toBeUndefined();
      expect(node.subject).toBeUndefined();
      expect(node.value).toBeUndefined();
    });

    it.each([['EQUALS'], ['AND'], ['IF'], ['NOT'], ['ADD'], ['AGE'], ['DATE_DIFF']])(
      'strips FOREACH fields when switching to %s',
      (newType) => {
        const node: TestNode = { ...foreachNode(), filter: { operation: 'NOT', value: '$x' } };
        const wrapper = mountOp(node);

        wrapper.vm.changeOperationType(newType);

        expect(node.operation).toBe(newType);
        // `additionalProperties: false` on every operation type means a leaked
        // collection/as/body would fail validation on save.
        expect(node.collection).toBeUndefined();
        expect(node.as).toBeUndefined();
        expect(node.body).toBeUndefined();
        expect(node.filter).toBeUndefined();
        expect(node.combine).toBeUndefined();
      },
    );
  });

  describe('applyValueMutation', () => {
    it('writes every FOREACH slot on the node', () => {
      const node: TestNode = { operation: 'FOREACH', collection: '', as: 'item', body: '' };
      const wrapper = mountOp(node);
      wrapper.vm.applyValueMutation({ _kind: 'collection' }, '$kinderen');
      wrapper.vm.applyValueMutation({ _kind: 'as' }, 'kind');
      wrapper.vm.applyValueMutation({ _kind: 'body' }, '$kind.bedrag');
      wrapper.vm.applyValueMutation({ _kind: 'combine' }, 'MAX');
      expect(node.collection).toBe('$kinderen');
      expect(node.as).toBe('kind');
      expect(node.body).toBe('$kind.bedrag');
      expect(node.combine).toBe('MAX');
    });
  });

  describe('canRemoveValue', () => {
    it('blocks the three required slots and allows the two optional ones', () => {
      const wrapper = mountOp(foreachNode());
      expect(wrapper.vm.canRemoveValue({ _kind: 'collection' })).toBe(false);
      expect(wrapper.vm.canRemoveValue({ _kind: 'as' })).toBe(false);
      expect(wrapper.vm.canRemoveValue({ _kind: 'body' })).toBe(false);
      expect(wrapper.vm.canRemoveValue({ _kind: 'filter' })).toBe(true);
      expect(wrapper.vm.canRemoveValue({ _kind: 'combine' })).toBe(true);
    });
  });

  describe('canAddValue', () => {
    it('disables the add-value button for FOREACH', () => {
      expect(mountOp(foreachNode()).vm.canAddValue).toBe(false);
    });
  });

  describe('canChangeValueKind', () => {
    it('keeps the binding and combine literal-only', () => {
      const wrapper = mountOp(foreachNode());
      expect(wrapper.vm.canChangeValueKind({ _kind: 'collection' })).toBe(true);
      expect(wrapper.vm.canChangeValueKind({ _kind: 'body' })).toBe(true);
      expect(wrapper.vm.canChangeValueKind({ _kind: 'filter' })).toBe(true);
      // `as` is an identifier and `combine` an enum from a fixed list.
      expect(wrapper.vm.canChangeValueKind({ _kind: 'as' })).toBe(false);
      expect(wrapper.vm.canChangeValueKind({ _kind: 'combine' })).toBe(false);
    });
  });
});

describe('OperationSettings - RECORD', () => {
  // The panel cannot edit `fields` yet, so it never turns another operation
  // into a RECORD; a RECORD that is there still shows its own type.
  it('offers RECORD only on a RECORD', () => {
    const values = (node: TestNode) => mountOp(node).vm.typeOptions.map((o) => o.value);
    expect(values({ operation: 'ADD', values: [1, 2] })).not.toContain('RECORD');
    expect(values({ operation: 'RECORD', fields: { naam: '$n' } })).toContain('RECORD');
  });
});
