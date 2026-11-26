<script setup lang="ts">
// The standing sign in the top bar that something the editor reported is still
// wrong (errors store). The notification says so once and can be closed; this
// stays until the cause is gone, and opens the same details sheet. It is in
// the top bar because an error belongs to the screen, not to one pane, and the
// bar is there in every pane layout.
//
// Rendered twice per bar, as the shell does for its other items: the button,
// and the menu item the toolbar shows when the button does not fit.
import { computed, nextTick, ref, watch } from 'vue';
import { storeToRefs } from 'pinia';
import { useErrorsStore } from '../stores/errors';

defineProps<{ menuItem?: boolean }>();

const store = useErrorsStore();
const { standing, detailsOpen } = storeToRefs(store);

const label = computed(() => (standing.value.length === 1 ? '1 fout' : `${standing.value.length} fouten`));

// Set by whichever copy opened the sheet, so that one gets the focus back.
const openedHere = ref(false);
const button = ref<HTMLElement | null>(null);

function open(): void {
  openedHere.value = true;
  store.showDetails(standing.value);
}

// Closing the sheet puts the focus back where the user came from. Opened from
// the notification there is no such place left, so the visible indicator takes
// it: the one thing on screen that still stands for the error.
watch(detailsOpen, async (isOpen) => {
  if (isOpen) return;
  const cameFromHere = openedHere.value;
  openedHere.value = false;
  await nextTick();
  const el = button.value;
  if (!el || el.getClientRects().length === 0) return;
  if (cameFromHere || document.activeElement === document.body) el.focus();
});
</script>

<template>
  <nldd-menu-item v-if="menuItem" icon="error" :text="label" data-testid="error-indicator-menu-item" @select="open"></nldd-menu-item>
  <nldd-button
    v-else
    ref="button"
    size="md"
    start-icon="error"
    :text="label"
    :accessible-label="`${label}. Bekijk de meldingen`"
    data-testid="error-indicator"
    @click="open"
  ></nldd-button>
</template>
