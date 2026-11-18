<script setup lang="ts">
import { computed } from 'vue';
import { isNullText } from '../utils/nullability';

// The one control for stating an absence (RFC-036): a checkbox "afwezig"
// next to a field the law declares `nullable: true`. Checked, the field
// holds the word `null`; unchecked, it holds a blank ("no value stated",
// unknown). The two are different cells and the form keeps them apart: a
// blank is what clearing the field gives, a `null` is a statement the author
// makes here, on purpose, whatever the field's datatype. A number, amount or
// date control cannot hold the word `null`, and a switch cannot show a third
// state, so without this control those fields have no way to state one.
//
// Purely presentational, like ScenarioParameterInput: the parent owns the
// value and decides whether the null is allowed (the parent only renders
// this control where the declaration says it is).
const props = withDefaults(defineProps<{
  /** The field's stored value; the word `null` (or a JS null) means checked */
  value?: unknown;
  /** Disabled state, for a read-only table */
  disabled?: boolean;
}>(), {
  // A factory only because the type is `unknown`; Vue calls it, so the
  // default is still ''.
  value: () => '',
  disabled: false,
});

const emit = defineEmits<{
  update: [value: 'null' | ''];
}>();

const absent = computed(() => isNullText(props.value));

function onChange(event: Event) {
  emit('update', (event as CustomEvent<{ checked?: boolean } | undefined>).detail?.checked ? 'null' : '');
}
</script>

<template>
  <nldd-checkbox-field
    label="afwezig"
    :checked="absent || undefined"
    :disabled="disabled || undefined"
    @change="onChange"
  ></nldd-checkbox-field>
</template>
