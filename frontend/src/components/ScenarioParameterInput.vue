<script setup lang="ts">
import { computed } from 'vue';
import { centsToEuros, eurosToCents } from '../utils/currency';
import { isNullText } from '../utils/nullability';

// Generic, datatype-driven scenario input control. Given a declared datatype
// (and optional unit), it renders the matching NDD component and emits a
// correctly-typed JS value via `update`. It stays purely presentational: the
// parent owns the value store and the `change`/auto-execute side effects.
//
// Round-trip is handled here: scenario values often arrive as strings parsed
// from Gherkin ("true" / "1500" / "2025-01-01"), and downstream consumers
// (engine execute + Gherkin re-serialisation) accept real JS types - so we
// coerce to the right type on the way out. See ScenarioForm.execute() and
// formMapper.syncEditedValues().
const props = withDefaults(defineProps<{
  /** Declared datatype: string | number | boolean | amount | date */
  type?: string;
  /** type_spec.unit for amounts (e.g. 'eurocent' | 'euro'); null when unannotated */
  unit?: string | null;
  /** Raw stored value (string or already-typed) */
  value?: unknown;
  /** Parameter name (used as the boolean switch label) */
  name?: string;
  /** Mark the field invalid */
  invalid?: boolean;
  /** Ids of the nldd-validation-items this value fails, set as `unmet` on the control */
  unmet?: string;
}>(), {
  type: 'string',
  unit: null,
  // A factory: `value` is typed `unknown`, so withDefaults wants one; Vue calls it
  // for an untyped prop, which yields the same '' the runtime default gave.
  value: () => '',
  name: '',
  invalid: false,
  unmet: undefined,
});

const emit = defineEmits<{
  update: [value: unknown];
}>();

// A stated absence (RFC-036): the value is the word `null`, or a JS null
// from a typed record. It shows as the word `null`, in a text control
// whatever the declared type - a number field cannot hold it and a switch
// cannot show it. The text stays editable: typing over it is one way out,
// the AbsenceToggle next to the field (rendered by the parent where the law
// allows an absence) is the other.
const showsNull = computed(() => isNullText(props.value));

// Amounts whose unit is eurocent are stored as integer cents but entered in
// euros. Any other amount (unit 'euro' or unannotated) is entered raw, so we
// never silently scale a value whose unit we don't actually know.
const isEurocent = computed(() => props.type === 'amount' && props.unit === 'eurocent');

// Interpret the incoming (possibly string) value for display per datatype.
const displayValue = computed(() => {
  const v = props.value;
  switch (props.type) {
    case 'boolean':
      return v === true || v === 'true' || v === '1';
    case 'number':
      return v === '' || v == null ? '' : Number(v);
    case 'amount':
      if (v === '' || v == null) return '';
      return isEurocent.value ? centsToEuros(v) : Number(v);
    default:
      // date + string
      return String(v ?? '');
  }
});

function emitNumber(detailValue: unknown) {
  emit('update', detailValue === '' || detailValue == null ? '' : Number(detailValue));
}

function emitAmount(detailValue: unknown) {
  if (detailValue === '' || detailValue == null) return emit('update', '');
  emit('update', isEurocent.value ? eurosToCents(detailValue) : Number(detailValue));
}

// The nldd-* fields' events, read the way the inline handlers did: a text
// field's value from its target (falling back to a CustomEvent detail), a
// number field's and a switch's from the detail.
type FieldEvent = Event & { detail?: { value?: unknown; checked?: unknown } | null };

function onTextInput(event: FieldEvent) {
  emit('update', (event.target as { value?: unknown } | null)?.value ?? event.detail?.value ?? '');
}

function onDateInput(event: FieldEvent) {
  emit('update', event.detail?.value ?? '');
}

function onSwitchChange(event: FieldEvent) {
  emit('update', Boolean(event.detail?.checked));
}
</script>

<template>
  <!-- a stated absence -> the word null, in a text field whatever the type -->
  <nldd-text-field
    v-if="showsNull"
    size="md"
    :invalid="invalid || undefined"
    :unmet="invalid ? unmet : undefined"
    value="null"
    @input="onTextInput"
  ></nldd-text-field>

  <!-- boolean -> switch (consistent with EditSheet's boolean control) -->
  <nldd-switch-field
    v-else-if="type === 'boolean'"
    :checked="displayValue ? true : undefined"
    :invalid="invalid || undefined"
    :unmet="invalid ? unmet : undefined"
    @change="onSwitchChange"
  >{{ name }}</nldd-switch-field>

  <!-- amount -> number-field; enters euros (eurocent unit) or raw otherwise.
       Both @input and @change are intentional: @input gives live updates,
       @change delivers the locale-normalised value on commit/blur. -->
  <nldd-number-field
    v-else-if="type === 'amount'"
    :value="displayValue"
    :invalid="invalid || undefined"
    :unmet="invalid ? unmet : undefined"
    step="0.01"
    width="full"
    hide-spin-buttons
    @input="(e: FieldEvent) => emitAmount(e.detail?.value)"
    @change="(e: FieldEvent) => emitAmount(e.detail?.value)"
  ></nldd-number-field>

  <!-- number -> number-field -->
  <nldd-number-field
    v-else-if="type === 'number'"
    :value="displayValue"
    :invalid="invalid || undefined"
    :unmet="invalid ? unmet : undefined"
    width="full"
    hide-spin-buttons
    @input="(e: FieldEvent) => emitNumber(e.detail?.value)"
    @change="(e: FieldEvent) => emitNumber(e.detail?.value)"
  ></nldd-number-field>

  <!-- date -> date-field: ISO (yyyy-mm-dd) in and out, '' while the typed
       date is not complete. Typing fires `input`; a date picked in the
       calendar fires only `change`, so both are handled. -->
  <nldd-date-field
    v-else-if="type === 'date'"
    :invalid="invalid || undefined"
    :unmet="invalid ? unmet : undefined"
    :value="displayValue"
    @input="onDateInput"
    @change="onDateInput"
  ></nldd-date-field>

  <!-- string (fallback) -->
  <nldd-text-field
    v-else
    size="md"
    :invalid="invalid || undefined"
    :unmet="invalid ? unmet : undefined"
    :value="displayValue"
    @input="onTextInput"
  ></nldd-text-field>
</template>
