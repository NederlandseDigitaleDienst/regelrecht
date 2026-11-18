<script setup lang="ts">
/**
 * Pane-level failure of the committed-notes resolve: the sidecar fetch, the
 * engine load or the resolver itself threw (e.g. the viewed law version is
 * not loaded in the engine). Without this dialog the notes pane silently fell
 * through to "Geen notities voor dit artikel", which reads as an empty
 * sidecar instead of a failure - the silent wrong answer RFC-019 forbids.
 *
 * `error` is whatever useNotes caught: an Error, or the bare string a WASM
 * throw produces.
 */
import { computed } from 'vue';

const props = withDefaults(defineProps<{
  // An Error (or Error-like object) or a bare string.
  error?: { message?: string } | string | null;
}>(), {
  error: null,
});

const supportingText = computed(() => {
  const e = props.error;
  if (!e) return '';
  return typeof e === 'string' ? e : e.message || String(e);
});
</script>

<template>
  <nldd-inline-dialog
    v-if="error"
    variant="alert"
    data-testid="notes-load-error"
    text="Notities laden mislukt"
    :supporting-text="supportingText"
  ></nldd-inline-dialog>
</template>
