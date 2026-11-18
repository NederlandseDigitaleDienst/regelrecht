<template>
  <!-- The button carries the state ("2 van 3"), so it says something while
       closed. The variants themselves are in the menu, because their titles
       are half a line long and more can be added: a row of chips grows with
       their number, a button does not. -->
  <nldd-button
    expandable
    appearance="neutral-tinted"
    size="sm"
    :text="buttonText"
    :disabled="!variants.length ? true : undefined"
  >
    <nldd-menu slot="popup" max-items="8" @select="onSelect">
      <nldd-menu-item
        v-for="v in variants"
        :key="v.id"
        type="checkbox"
        :text="titleOf(v)"
        :value="v.id"
        :selected="selected.includes(v.id) ? true : undefined"
        :disabled="!selected.includes(v.id) && selected.length >= max ? true : undefined"
      ></nldd-menu-item>
      <span v-if="footer" slot="footer" class="column-menu-footer">{{ footer }}</span>
    </nldd-menu>
  </nldd-button>
</template>

<!--
  Column picker, next to the table it fills.

  The choice used to sit in the left panel, while everything it decides (the
  table and the two charts) is on the right. The left is about what you
  change, and this changes nothing: it picks what is computed next to the law
  in force.

  Why a menu and not a row of chips: the number of variants is not fixed. Next
  to the ones from the case, "Bewaar als variant" makes new ones, with a title
  the user chooses. Chips would make the header grow with every variant, and
  their labels were already cut off to "nk-2: één definitie van…". The menu
  fits the whole title and the button keeps its width.
-->

<script setup lang="ts" generic="V extends { id: string }">
import { computed } from 'vue';

const props = withDefaults(
  defineProps<{
    /** Every available variant, each with an `id`. */
    variants: V[];
    /** The selected variant ids, in the order they were picked. */
    selected: string[];
    /** How many variants fit next to the law in force. */
    max?: number;
    /** How a variant writes its name; slightly different per case. */
    titleOf: (variant: V) => string;
    /**
     * A short line under the menu, for what the list does not tell. Keep it
     * short: it partly sets the menu's height, and nobody reads a paragraph
     * here. May be left empty.
     */
    footer?: string;
  }>(),
  { max: 3, footer: '' },
);
const emit = defineEmits<{ 'update:selected': [selected: string[]] }>();

/**
 * Closed, the button has to say what is shown. "Kolommen" alone would be a
 * button whose state you cannot see without opening it, which is exactly
 * what the panel before it could do.
 */
const buttonText = computed(() => {
  if (!props.variants.length) return 'Kolommen: geen varianten';
  if (!props.selected.length) return `Kolommen: alleen huidig recht`;
  return `Kolommen: ${props.selected.length} van ${props.max}`;
});

/**
 * The menu item says which item was clicked; the selection state comes from
 * us, so the maximum holds even when items are clicked in quick succession.
 */
function onSelect(event: Event) {
  const item = (event.target as Element | null)?.closest<HTMLElement & { value?: unknown }>('nldd-menu-item');
  if (!item) return;
  const id = String(item.value);
  const isOn = props.selected.includes(id);
  if (!isOn && props.selected.length >= props.max) return;
  emit('update:selected', isOn ? props.selected.filter((x) => x !== id) : [...props.selected, id]);
}
</script>

<style scoped>
/* The footer line is the explanation that used to sit under the panel. The
   menu leaves its footer unstyled, so the margin and the tone come from here. */
.column-menu-footer {
  display: block;
  padding: var(--primitives-space-4) var(--primitives-space-12) var(--primitives-space-8);
  font-size: 0.8em;
  line-height: 1.3;
  color: var(--semantics-content-secondary-color);
}
</style>
