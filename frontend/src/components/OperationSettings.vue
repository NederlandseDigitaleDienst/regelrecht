<script setup lang="ts">
import { computed } from 'vue';
import {
  OPERATION_LABELS,
  collectAvailableVariables,
  derivedTitle,
  type OperationNode,
  type OperationTreeRow,
} from '../utils/operationTree';
import BreakableName from './BreakableName.vue';

/** The parts of an article this component reads (via collectAvailableVariables). */
export interface OperationSettingsArticle {
  machine_readable?: {
    definitions?: Record<string, unknown> | null;
    execution?: {
      input?: { name: string }[] | null;
      parameters?: { name: string }[] | null;
      actions?: { output?: string }[] | null;
    } | null;
  } | null;
}

/**
 * The operation node as this component writes it. The form stores whatever
 * the author typed or picked (parseInputValue may give a number or boolean),
 * also in `in`, `as` and `combine`, which OperationNode declares as strings.
 */
type EditableNode = Omit<OperationNode, 'in' | 'as' | 'combine'> & {
  in?: unknown;
  as?: unknown;
  combine?: unknown;
};

/** Which slot of the node a value row reads and writes. */
type ValueKind =
  | 'subject' | 'value' | 'date_of_birth' | 'reference_date'
  | 'from' | 'to' | 'in' | 'collection' | 'as' | 'filter' | 'body' | 'combine'
  | 'values' | 'conditions' | 'case-when' | 'case-then' | 'default';

/** One row of the value list, built by operationValues. */
interface ValueRow {
  _label: string;
  _value: unknown;
  _kind: ValueKind;
  _index?: number;
  _caseIndex?: number;
}

const props = withDefaults(defineProps<{
  operation?: OperationTreeRow | null;
  article?: OperationSettingsArticle | null;
  editable?: boolean;
  // Suppress the read-only "Titel" row. The action sheet sets this at the
  // top level in edit mode, where the editable Output field takes that
  // first/name position instead (avoiding a duplicate name row).
  hideTitleRow?: boolean;
}>(), {
  operation: null,
  article: null,
  editable: false,
  hideTitleRow: false,
});

const emit = defineEmits<{
  'select-operation': [node: OperationNode];
}>();

const availableVariables = computed(() => collectAvailableVariables(props.article));

const typeOptions = computed(() =>
  Object.entries(OPERATION_LABELS).map(([key, label]) => ({ value: key, label }))
);

// Group variables by category with alphabetical sorting within each group,
// matching the optgroup pattern used in EditSheet's source parameter dropdown.
const variableGroups = computed(() => {
  const groups = new Map<string, { value: string; label: string }[]>();
  for (const v of availableVariables.value) {
    if (!groups.has(v.category)) groups.set(v.category, []);
    // Set on the line above when missing.
    groups.get(v.category)!.push({
      value: v.ref,
      label: v.name,
    });
  }
  for (const opts of groups.values()) {
    opts.sort((a, b) => a.label.localeCompare(b.label, 'nl'));
  }
  return groups;
});

const COMPARISON_OPS = new Set([
  'EQUALS', 'NOT_EQUALS', 'GREATER_THAN', 'GREATER_THAN_OR_EQUAL',
  'LESS_THAN', 'LESS_THAN_OR_EQUAL', 'NOT_NULL', 'IN', 'NOT_IN',
]);

const LOGICAL_OPS = new Set(['AND', 'OR']);
const ARITHMETIC_OPS = new Set(['ADD', 'SUBTRACT', 'MULTIPLY', 'DIVIDE', 'MIN', 'MAX', 'CONCAT']);

// `?? ''` only for the type: '' is not in the set, as undefined was not.
const isComparisonOp = computed(() => COMPARISON_OPS.has(props.operation?.operation ?? ''));

const canAddValue = computed(() => {
  if (!props.editable) return false;
  const op = props.operation?.operation;
  if (!op) return false;
  // Structural-slot ops have no concept of "add a value"
  if (op === 'NOT' || op === 'IF' || op === 'SWITCH') return false;
  // NOT_NULL never takes a value field (it only checks the subject is non-null)
  if (op === 'NOT_NULL') return false;
  // AGE has a fixed shape (date_of_birth + reference_date) - no add slot.
  if (op === 'AGE') return false;
  // DATE_DIFF has a fixed shape (from + to + in) - no add slot.
  if (op === 'DATE_DIFF') return false;
  // FOREACH has a fixed shape (collection + as + body, optional filter/combine).
  if (op === 'FOREACH') return false;
  // Comparison ops always have exactly subject + value (or just subject for
  // NOT_NULL); operationValues pushes both unconditionally, so addValue() has
  // nothing to do. Hide the button to avoid a no-op click.
  if (COMPARISON_OPS.has(op)) return false;
  // Logical ops only accept nested operations as conditions; "Voeg waarde toe"
  // would push an EQUALS predicate identical to "Voeg operatie toe", so we
  // suppress the duplicate button.
  if (LOGICAL_OPS.has(op)) return false;
  return true;
});

// Required structural fields whose minus button must be hidden so the user
// cannot delete them and silently produce an invalid node.
function canRemoveValue(val: ValueRow) {
  if (!props.editable) return false;
  const node = props.operation?.node;
  const op = props.operation?.operation;
  if (isComparisonOp.value && (val._kind === 'subject' || val._kind === 'value')) return false;
  // NOT needs value
  if (op === 'NOT' && val._kind === 'value') return false;
  // AGE has two fixed structural slots - neither can be deleted.
  if (op === 'AGE' && (val._kind === 'date_of_birth' || val._kind === 'reference_date')) return false;
  if (op === 'DATE_DIFF' && (val._kind === 'from' || val._kind === 'to' || val._kind === 'in')) return false;
  // FOREACH: collection, as and body are required; filter and combine are optional.
  if (op === 'FOREACH' && (val._kind === 'collection' || val._kind === 'as' || val._kind === 'body')) return false;
  // IF and SWITCH share the cases[]/default schema and both need a default branch
  if ((op === 'IF' || op === 'SWITCH') && val._kind === 'default') return false;
  // IF must keep its single case; SWITCH must keep at least one case.
  // Removing the case-when or case-then deletes the whole case entry.
  if ((val._kind === 'case-when' || val._kind === 'case-then') && Array.isArray(node?.cases)) {
    if (op === 'IF') return false;
    if (op === 'SWITCH' && node.cases.length <= 1) return false;
  }
  // AND/OR/arithmetic ops need at least one entry - block removal of the
  // last condition or value so the user can't drain conditions: [] / values: []
  // and produce a semantically undefined node.
  if (val._kind === 'conditions' && Array.isArray(node?.conditions) && node.conditions.length <= 1) return false;
  if (val._kind === 'values' && Array.isArray(node?.values) && node.values.length <= 1) return false;
  return true;
}

const operationValues = computed((): ValueRow[] => {
  const node: EditableNode | undefined = props.operation?.node;
  if (!node) return [];

  if (isComparisonOp.value) {
    const vals: ValueRow[] = [];
    vals.push({ _label: 'Onderwerp', _value: node.subject ?? '', _kind: 'subject' });
    if (node.operation !== 'NOT_NULL') {
      vals.push({ _label: 'Waarde', _value: node.value ?? '', _kind: 'value' });
    }
    return vals;
  }

  // AGE: two structural slots, similar in shape to a comparison op (fixed,
  // named) but with date semantics. Returns the age in whole years.
  if (node.operation === 'AGE') {
    return [
      { _label: 'Geboortedatum', _value: node.date_of_birth ?? '', _kind: 'date_of_birth' },
      { _label: 'Peildatum', _value: node.reference_date ?? '', _kind: 'reference_date' },
    ];
  }

  // DATE_DIFF: three fixed structural slots (RFC-021). The unit (`in`) is an
  // enum value or a variable reference, never a nested operation.
  if (node.operation === 'DATE_DIFF') {
    return [
      { _label: 'Van', _value: node.from ?? '', _kind: 'from' },
      { _label: 'Tot', _value: node.to ?? '', _kind: 'to' },
      { _label: 'Eenheid', _value: node.in ?? '', _kind: 'in' },
    ];
  }

  // FOREACH: the collection, the binding name, the body, and two optional
  // slots (RFC-016). `as` is an identifier and `combine` an enum from a fixed
  // list; neither is ever a nested operation.
  if (node.operation === 'FOREACH') {
    const vals: ValueRow[] = [
      { _label: 'Verzameling', _value: node.collection ?? '', _kind: 'collection' },
      { _label: 'Naam per element', _value: node.as ?? '', _kind: 'as' },
    ];
    if (node.filter !== undefined) {
      vals.push({ _label: 'Filter', _value: node.filter, _kind: 'filter' });
    }
    vals.push({ _label: 'Per element', _value: node.body ?? '', _kind: 'body' });
    if (node.combine !== undefined) {
      vals.push({ _label: 'Samenvoegen', _value: node.combine, _kind: 'combine' });
    }
    return vals;
  }

  // IF and SWITCH share the same cases[]/default schema. The only difference
  // is semantic (IF is single-case, SWITCH is multi-case) and how the user
  // labels each branch.
  if (node.operation === 'IF' || node.operation === 'SWITCH') {
    const vals: ValueRow[] = [];
    const isSwitch = node.operation === 'SWITCH';
    if (Array.isArray(node.cases)) {
      node.cases.forEach((c, i) => {
        const prefix = isSwitch ? `Geval ${i + 1} - ` : '';
        if (c?.when !== undefined) vals.push({ _label: `${prefix}Als`, _value: c.when, _kind: 'case-when', _caseIndex: i });
        if (c?.then !== undefined) vals.push({ _label: `${prefix}Dan`, _value: c.then, _kind: 'case-then', _caseIndex: i });
      });
    }
    if (node.default !== undefined) vals.push({ _label: isSwitch ? 'Standaard' : 'Anders', _value: node.default, _kind: 'default' });
    return vals;
  }

  if (Array.isArray(node.values)) {
    return node.values.map((v, i) => ({ _label: `Waarde ${i + 1}`, _value: v, _kind: 'values', _index: i }));
  }
  if (Array.isArray(node.conditions)) {
    return node.conditions.map((c, i) => ({ _label: `Conditie ${i + 1}`, _value: c, _kind: 'conditions', _index: i }));
  }

  const vals: ValueRow[] = [];
  if (node.subject != null) vals.push({ _label: 'Onderwerp', _value: node.subject, _kind: 'subject' });
  if (node.value !== undefined) vals.push({ _label: 'Waarde', _value: node.value, _kind: 'value' });
  return vals;
});

// Not a type guard: it returns the `operation` value itself (truthy) rather
// than a boolean, and templates bind that result directly.
function isNestedOperation(val: unknown) {
  return val != null && typeof val === 'object' && (val as { operation?: unknown }).operation;
}

function isLiteralValue(val: unknown) {
  return val === null || typeof val === 'number' || typeof val === 'boolean' || (typeof val === 'string' && !val.startsWith('$'));
}

// Mirror the read-only Waarde row (readonlyValueText): show the nested op's
// own title when the author supplied one, falling back to the derived
// expression. Keeps the dropdown label identical to the Titel row the user
// sees after clicking "Bewerken" into that operation.
function valueDropdownNestedOption(val: unknown) {
  if (!isNestedOperation(val)) return null;
  const node = val as OperationNode;
  return { value: '__nested__', label: `${node.title ?? derivedTitle(node)} (operatie)` };
}

function currentDropdownValue(val: unknown) {
  if (isNestedOperation(val)) return '__nested__';
  if (typeof val === 'string' && val.startsWith('$')) return val;
  return String(val);
}

// --- Mutation helpers ---

function parseInputValue(str: string) {
  if (str === 'true') return true;
  if (str === 'false') return false;
  const n = Number(str);
  if (!isNaN(n) && str.trim() !== '') return n;
  return str;
}

function changeOperationType(newType: string) {
  const node: EditableNode | undefined = props.operation?.node;
  if (!node) return;
  const oldType = node.operation;
  if (!newType || newType === oldType) return;

  node.operation = newType;

  // Every non-AGE branch must also strip `date_of_birth` / `reference_date`
  // so a transition out of AGE doesn't leave the slots dangling on the
  // node. The schema marks every operation type as
  // `additionalProperties: false`, so a leaked field would fail
  // validation on save and corrupt the law.
  if (COMPARISON_OPS.has(newType)) {
    if (node.subject === undefined) node.subject = '';
    if (newType === 'NOT_NULL') {
      delete node.value;
    } else {
      // For all other comparison ops (including IN/NOT_IN), seed value as
      // an empty string. The UI dropdown lets the user pick a variable
      // reference that resolves to a list at runtime; arrays as literal
      // values can be entered through the YAML pane.
      if (node.value === undefined || node.value === null) node.value = '';
    }
    delete node.values;
    delete node.conditions;
    delete node.cases;
    delete node.default;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
    delete node.from;
    delete node.to;
    delete node.in;
  } else if (LOGICAL_OPS.has(newType)) {
    if (!Array.isArray(node.conditions)) {
      node.conditions = [];
    }
    delete node.values;
    delete node.subject;
    delete node.value;
    delete node.cases;
    delete node.default;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
    delete node.from;
    delete node.to;
    delete node.in;
  } else if (newType === 'IF') {
    // IF uses the same cases[]/default schema as SWITCH but is single-case.
    // Truncate any extra cases when transitioning from SWITCH so we don't
    // produce an IF with 2+ cases that the schema would reject.
    if (!Array.isArray(node.cases) || node.cases.length === 0) {
      node.cases = [{ when: { operation: 'EQUALS', subject: '', value: '' }, then: 0 }];
    } else if (node.cases.length > 1) {
      node.cases = node.cases.slice(0, 1);
    }
    if (node.default === undefined) node.default = 0;
    delete node.values;
    delete node.conditions;
    delete node.subject;
    delete node.value;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
    delete node.from;
    delete node.to;
    delete node.in;
  } else if (newType === 'NOT') {
    if (node.value === undefined) {
      node.value = node.subject ?? '';
    }
    delete node.values;
    delete node.conditions;
    delete node.cases;
    delete node.default;
    delete node.subject;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
    delete node.from;
    delete node.to;
    delete node.in;
  } else if (newType === 'SWITCH') {
    // Schema requires at least one case
    if (!Array.isArray(node.cases) || node.cases.length === 0) {
      node.cases = [{ when: { operation: 'EQUALS', subject: '', value: '' }, then: 0 }];
    }
    if (node.default === undefined) node.default = 0;
    delete node.values;
    delete node.conditions;
    delete node.subject;
    delete node.value;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
    delete node.from;
    delete node.to;
    delete node.in;
  } else if (ARITHMETIC_OPS.has(newType)) {
    if (!Array.isArray(node.values)) {
      node.values = [];
    }
    delete node.conditions;
    delete node.cases;
    delete node.default;
    delete node.subject;
    delete node.value;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
    delete node.from;
    delete node.to;
    delete node.in;
  } else if (newType === 'AGE') {
    // AGE has two fixed structural slots - seed both as empty strings so
    // the user can fill them via the form. Strip every other slot so the
    // node is shaped exactly like the engine's `ActionOperation::Age`.
    if (node.date_of_birth === undefined) node.date_of_birth = '';
    if (node.reference_date === undefined) node.reference_date = '';
    delete node.subject;
    delete node.value;
    delete node.values;
    delete node.conditions;
    delete node.cases;
    delete node.default;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.from;
    delete node.to;
    delete node.in;
  } else if (newType === 'DATE_DIFF') {
    // DATE_DIFF has three fixed structural slots (RFC-021). Seed `in` with
    // a valid unit so the node validates without further input.
    if (node.from === undefined) node.from = '';
    if (node.to === undefined) node.to = '';
    if (node.in === undefined) node.in = 'days';
    delete node.subject;
    delete node.value;
    delete node.values;
    delete node.conditions;
    delete node.cases;
    delete node.default;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
  } else if (newType === 'FOREACH') {
    // FOREACH needs a collection, a binding and a body (RFC-016). Seed `as`
    // with the schema default so the node validates without further input;
    // `filter` and `combine` stay absent until the author adds them.
    if (node.collection === undefined) node.collection = '';
    if (node.as === undefined) node.as = 'item';
    if (node.body === undefined) node.body = '';
    delete node.subject;
    delete node.value;
    delete node.values;
    delete node.conditions;
    delete node.cases;
    delete node.default;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.from;
    delete node.to;
    delete node.in;
    delete node.date_of_birth;
    delete node.reference_date;
  }

  // Switching to anything else drops the FOREACH-only fields, so a leftover
  // `as` or `combine` cannot ride along into a node whose schema forbids it
  // (every operation is additionalProperties: false).
  if (newType !== 'FOREACH') {
    delete node.collection;
    delete node.as;
    delete node.body;
    delete node.filter;
    delete node.combine;
  }
}

function applyValueMutation(val: ValueRow, newVal: unknown) {
  const node: EditableNode | undefined = props.operation?.node;
  if (!node) return;
  if (val._kind === 'subject') node.subject = newVal;
  else if (val._kind === 'value') node.value = newVal;
  else if (val._kind === 'date_of_birth') node.date_of_birth = newVal;
  else if (val._kind === 'reference_date') node.reference_date = newVal;
  else if (val._kind === 'from') node.from = newVal;
  else if (val._kind === 'to') node.to = newVal;
  else if (val._kind === 'in') node.in = newVal;
  else if (val._kind === 'collection') node.collection = newVal;
  else if (val._kind === 'as') node.as = newVal;
  else if (val._kind === 'body') node.body = newVal;
  else if (val._kind === 'filter') node.filter = newVal;
  else if (val._kind === 'combine') node.combine = newVal;
  // A 'values' / 'conditions' row is only built from an array on this node,
  // and without one this threw before, as it still does.
  else if (val._kind === 'values' && val._index !== undefined) node.values![val._index] = newVal;
  else if (val._kind === 'conditions' && val._index !== undefined) node.conditions![val._index] = newVal;
  else if (val._kind === 'default') node.default = newVal;
  else if (val._kind === 'case-when' && val._caseIndex !== undefined && Array.isArray(node.cases)) {
    // _caseIndex is the index of this case when the row was built.
    node.cases[val._caseIndex]!.when = newVal;
  } else if (val._kind === 'case-then' && val._caseIndex !== undefined && Array.isArray(node.cases)) {
    node.cases[val._caseIndex]!.then = newVal;
  }
}

function updateValue(val: ValueRow, event: Event) {
  const target = event.target as { value?: string } | null;
  const detail = (event as CustomEvent<{ value?: string } | undefined>).detail;
  const newVal = parseInputValue(target?.value ?? detail?.value ?? '');
  applyValueMutation(val, newVal);
}

function updateDropdownValue(val: ValueRow, event: Event) {
  const selected = (event.target as HTMLSelectElement).value;
  // The '__nested__' sentinel is just the current operation's own row in
  // the list - re-picking it means "keep the operation" (edit it via the
  // pencil), so it's a no-op. Picking anything else (a variable, literal,
  // or the empty option) replaces the value, including replacing a nested
  // operation - otherwise switching e.g. an IF op to $bsn silently does
  // nothing and never marks the action dirty. Consequence: picking the
  // empty "Selecteer…" option discards a nested op tree in one click,
  // same destructive trade as changeValueKind(); recovery is the
  // ActionSheet "Annuleren" snapshot-restore (smoke-test this path).
  if (selected === '__nested__') return;
  const newVal = selected.startsWith('$') ? selected : parseInputValue(selected);
  applyValueMutation(val, newVal);
}


// The Type radio (Waarde / Operatie) only applies to value slots that can
// hold either a literal or a nested operation - not subject/date pickers.
function canChangeValueKind(val: ValueRow) {
  return val._kind !== 'subject'
    && val._kind !== 'date_of_birth'
    && val._kind !== 'reference_date'
    // The DATE_DIFF unit is an enum or variable, never a nested operation.
    && val._kind !== 'in'
    // The FOREACH binding is an identifier and its combine an enum from a
    // fixed list; neither can hold a nested operation.
    && val._kind !== 'as'
    && val._kind !== 'combine';
}

// The actions menu only contains the Type group and Verwijder. When neither
// applies (e.g. a comparison op's fixed subject/value), the menu would render
// empty ("Geen opties beschikbaar"), so suppress the more-button entirely.
function hasValueMenu(val: ValueRow) {
  return canChangeValueKind(val) || canRemoveValue(val);
}

// Switch a value between a literal and a nested operation, writing through
// applyValueMutation so it lands in the correct slot for this _kind. The
// default operation mirrors the old "Operatie toevoegen" shape.
function changeValueKind(val: ValueRow, kind: 'value' | 'operatie') {
  const isOp = isNestedOperation(val._value);
  if (kind === 'operatie' && !isOp) {
    applyValueMutation(val, { operation: 'ADD', values: [] });
  } else if (kind === 'value' && isOp) {
    // Replaces the whole nested op with '' in one click. Deliberately
    // unconfirmed: this matches the equally-destructive sibling
    // removeValue() in the same menu - both rely on the ActionSheet
    // "Annuleren" snapshot-restore as the single, consistent undo,
    // rather than introducing a one-off confirm dialog here.
    applyValueMutation(val, '');
  }
}

function readonlyValueText(val: ValueRow) {
  const v = val._value;
  if (v === null || v === undefined || v === '') return '(leeg)';
  // For a nested op: prefer the user-supplied title; fall back to the
  // derived expression. Mirrors the buildOperationTree title rule so the
  // Waarde row reads identically to the Titel row after clicking in.
  if (isNestedOperation(v)) return (v as OperationNode).title ?? derivedTitle(v as OperationNode);
  // A YAML list renders as a value; String([...]) joins with a bare comma
  // ("a,b,c") which has no break opportunity and pushes the column width.
  // Use comma + space so it can wrap and reads naturally.
  if (Array.isArray(v)) return v.join(', ');
  return String(v);
}

function removeValue(val: ValueRow) {
  const node: EditableNode | undefined = props.operation?.node;
  if (!node) return;

  if (val._kind === 'values' && val._index !== undefined && Array.isArray(node.values)) {
    node.values.splice(val._index, 1);
  } else if (val._kind === 'conditions' && val._index !== undefined && Array.isArray(node.conditions)) {
    node.conditions.splice(val._index, 1);
  } else if (val._kind === 'subject') {
    delete node.subject;
  } else if (val._kind === 'value') {
    delete node.value;
  } else if ((val._kind === 'case-when' || val._kind === 'case-then') && val._caseIndex !== undefined && Array.isArray(node.cases)) {
    // Removing either side of a case entry removes the whole case
    node.cases.splice(val._caseIndex, 1);
  } else if (val._kind === 'default') {
    delete node.default;
  }
}

function addValue() {
  const node: EditableNode | undefined = props.operation?.node;
  if (!node) return;

  // Don't inject values[] into nodes with structural value slots
  // (NOT uses single 'value', IF uses when/then/else, SWITCH uses cases/default)
  if (node.operation === 'NOT' || node.operation === 'IF' || node.operation === 'SWITCH') return;
  // NOT_NULL is a unary check on subject only - never a value
  if (node.operation === 'NOT_NULL') return;

  if (Array.isArray(node.values)) {
    node.values.push(0);
  } else if (Array.isArray(node.conditions)) {
    node.conditions.push({ operation: 'EQUALS', subject: '', value: '' });
  } else if (isComparisonOp.value) {
    if (node.subject === undefined) node.subject = '';
    else if (node.value === undefined) node.value = '';
  } else {
    if (!node.values) node.values = [];
    node.values.push(0);
  }
}

</script>

<template>
  <template v-if="operation">
    <template v-if="String(operation.number).includes('.')">
      <nldd-title size="display-4" class="operation-settings__title">
        <h4>Operatie {{ operation.number }}</h4>
      </nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
    </template>
    <nldd-list appearance="box-tinted" class="settings-list">
      <!-- Optional lead row, rendered as the first item of THIS list so
           the action sheet's Output field sits in the same box as Type /
           conditions on the root of an action. -->
      <slot name="lead-row"></slot>
      <!-- Titel -->
      <nldd-list-item v-if="!hideTitleRow" size="md">
        <nldd-text-cell text="Titel" :width="editable ? '120px' : 'fit-content'"></nldd-text-cell>
        <nldd-spacer-cell size="12"></nldd-spacer-cell>
        <nldd-text-cell horizontal-alignment="right"><BreakableName :name="operation.title || '(leeg)'" /></nldd-text-cell>
      </nldd-list-item>

      <!-- Type -->
      <nldd-list-item size="md">
        <nldd-text-cell text="Type" :width="editable ? '120px' : 'fit-content'"></nldd-text-cell>
        <nldd-spacer-cell size="12"></nldd-spacer-cell>
        <nldd-cell v-if="editable">
          <nldd-dropdown size="md" data-testid="operation-type-dropdown">
            <!-- Same stale-collapsed-label issue as the value dropdown: the
                 <option> list (typeOptions) is constant, so navigating to a
                 different operation only changes the bound value, and a
                 native <select> reused in place keeps showing the previous
                 op's type label. Key on the operation identity + its type so
                 the element is recreated and the collapsed text stays in
                 sync with the selected option. -->
            <select
              :key="`op-type-${operation.number}-${operation.operation}`"
              aria-label="Operatie type"
              :value="operation.operation"
              @change="changeOperationType(($event.target as HTMLSelectElement).value)"
            >
              <option v-for="opt in typeOptions" :key="opt.value" :value="opt.value" :selected="opt.value === operation.operation">{{ opt.label }}</option>
            </select>
          </nldd-dropdown>
        </nldd-cell>
        <template v-else>
          <nldd-text-cell horizontal-alignment="right"><BreakableName :name="OPERATION_LABELS[operation.operation] || operation.operation || '(leeg)'" /></nldd-text-cell>
        </template>
      </nldd-list-item>

      <!-- Waarde rows -->
      <nldd-list-item
        v-for="(val, i) in operationValues"
        :key="i"
        size="md"
        :data-testid="`op-value-${i}`"
        :button="!editable && isNestedOperation(val._value)"
        @click="!editable && isNestedOperation(val._value) && emit('select-operation', val._value as OperationNode)"
      >
        <nldd-text-cell :text="val._label" :width="editable ? '120px' : 'fit-content'"></nldd-text-cell>
        <nldd-spacer-cell size="12"></nldd-spacer-cell>
        <template v-if="editable">
          <nldd-cell>
            <!-- Subject/date fields show a dropdown of available variables.
                 If the field already holds a literal value (e.g. "2025-01-01"),
                 it's included as the first option so it stays visible and
                 selectable. -->
            <template v-if="val._kind === 'subject' || val._kind === 'date_of_birth' || val._kind === 'reference_date'">
              <nldd-dropdown size="md">
                <select :aria-label="val._label" :value="currentDropdownValue(val._value)" @change="updateDropdownValue(val, $event)">
                  <option value="">Selecteer...</option>
                  <option v-if="isLiteralValue(val._value) && val._value !== '' && val._value !== null" :value="String(val._value)" :selected="true">{{ String(val._value) }}</option>
                  <optgroup v-for="[category, opts] in variableGroups" :key="category" :label="category">
                    <option v-for="opt in opts" :key="opt.value" :value="opt.value" :selected="opt.value === currentDropdownValue(val._value)">{{ opt.label }}</option>
                  </optgroup>
                </select>
              </nldd-dropdown>
            </template>
            <!-- Literal values show a text field -->
            <template v-else-if="isLiteralValue(val._value)">
              <nldd-text-field size="md" :value="String(val._value)" width="full" @input="updateValue(val, $event)"></nldd-text-field>
            </template>
            <!-- Variable references and nested operations show a full dropdown -->
            <template v-else>
              <nldd-dropdown size="md">
                <!-- A nested op's option value is the constant '__nested__',
                     and so is the select's bound value. Native <select>
                     only re-renders its COLLAPSED label when value changes,
                     so navigating to a different operation (which only swaps
                     the option TEXT, not its value) leaves the closed
                     control showing the previous op's label while the open
                     list shows the correct one. Keying the <select> on the
                     displayed identity forces a fresh element so the
                     collapsed text always matches the selected option. -->
                <select
                  :key="`val-sel-${i}-${valueDropdownNestedOption(val._value)?.label ?? currentDropdownValue(val._value)}`"
                  :aria-label="val._label"
                  :value="currentDropdownValue(val._value)"
                  @change="updateDropdownValue(val, $event)"
                >
                  <option value="">Selecteer...</option>
                  <template v-for="nestedOpt in [valueDropdownNestedOption(val._value)]" :key="nestedOpt?.label ?? 'nested'">
                    <option v-if="nestedOpt" :value="nestedOpt.value" :selected="currentDropdownValue(val._value) === '__nested__'">{{ nestedOpt.label }}</option>
                  </template>
                  <optgroup v-for="[category, opts] in variableGroups" :key="category" :label="category">
                    <option v-for="opt in opts" :key="opt.value" :value="opt.value" :selected="opt.value === currentDropdownValue(val._value)">{{ opt.label }}</option>
                  </optgroup>
                </select>
              </nldd-dropdown>
            </template>
          </nldd-cell>
          <template v-if="isNestedOperation(val._value)">
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-cell>
              <nldd-icon-button
                icon="edit"
                text="Bewerken"
                tooltip-timing="never"
                appearance="neutral-tinted"
                @click="emit('select-operation', val._value as OperationNode)"
              ></nldd-icon-button>
            </nldd-cell>
          </template>
          <template v-if="hasValueMenu(val)">
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <!-- The menu sits in the button's popup slot, so it anchors to this
                 exact button instead of to whichever element happens to own a
                 matching id — two panes showing the same operation used to
                 collide. -->
            <nldd-cell>
              <nldd-icon-button icon="more" text="Acties" tooltip-timing="never" appearance="neutral-tinted">
                <nldd-menu slot="popup">
                  <nldd-menu-group v-if="canChangeValueKind(val)" text="Type">
                    <nldd-menu-item
                      type="radio"
                      text="Waarde"
                      :selected="!isNestedOperation(val._value) || undefined"
                      @click.stop="changeValueKind(val, 'value')"
                    ></nldd-menu-item>
                    <nldd-menu-item
                      type="radio"
                      text="Operatie"
                      :selected="isNestedOperation(val._value) || undefined"
                      @click.stop="changeValueKind(val, 'operatie')"
                    ></nldd-menu-item>
                  </nldd-menu-group>
                  <nldd-menu-divider v-if="canChangeValueKind(val) && canRemoveValue(val)"></nldd-menu-divider>
                  <nldd-menu-item
                    v-if="canRemoveValue(val)"
                    text="Verwijder"
                    icon="delete"
                    destructive
                    @click.stop="removeValue(val)"
                  ></nldd-menu-item>
                </nldd-menu>
              </nldd-icon-button>
            </nldd-cell>
          </template>
        </template>
        <template v-else>
          <nldd-text-cell horizontal-alignment="right"><BreakableName :name="readonlyValueText(val)" /></nldd-text-cell>
          <template v-if="isNestedOperation(val._value)">
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-icon-cell size="20">
              <nldd-icon icon="chevron-right"></nldd-icon>
            </nldd-icon-cell>
          </template>
        </template>
      </nldd-list-item>

      <!-- Add value -->
      <nldd-list-item v-if="canAddValue" size="md">
        <nldd-cell width="full">
          <nldd-button width="full" size="md" start-icon="plus-small" data-testid="add-value-btn" @click="addValue" text="Voeg waarde toe"></nldd-button>
        </nldd-cell>
      </nldd-list-item>
    </nldd-list>
  </template>
</template>

<style>
/* Reset browser default h4 margin so the nldd-spacer below the title is the
 * only source of vertical gap. Without this the h4's default bottom margin
 * (~1em) collapses into the spacer unpredictably, which caused the
 * "textboxes falling under the title" visual in the action panel.
 *
 * Keyed off a `.operation-settings__title` BEM class on the nldd-title in
 * this component. The rule is in an unscoped `<style>` block alongside the
 * rest of the file's selectors (Vue scoped styles can't reach into NLDD
 * shadow DOM), so the class name is the only thing preventing bleed into
 * other components - keep the class unique to this component. */
.operation-settings__title h4 {
  margin: 0;
}

.settings-list nldd-cell {
  flex: 1;
  min-width: 0;
}
.settings-list nldd-text-field,
.settings-list nldd-dropdown {
  width: 100%;
}
</style>
