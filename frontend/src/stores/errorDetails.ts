import { ref } from 'vue';
import { defineStore } from 'pinia';
import type { UserError } from '../lib/userError';

/**
 * The technical-details sheet is one instance in the shell; any message with a
 * "Technische details" button opens it through this store.
 */
export const useErrorDetailsStore = defineStore('errorDetails', () => {
  const open = ref(false);
  // Kept after closing, so the text does not vanish during the close animation.
  const current = ref<UserError | null>(null);

  function show(error: UserError): void {
    current.value = error;
    open.value = true;
  }

  function close(): void {
    open.value = false;
  }

  return { open, current, show, close };
});
