import { ref } from 'vue';

const isOpen = ref(false);
const lastJobCreated = ref(0);

export function useNewHarvestJob() {
  return {
    isOpen,
    lastJobCreated,
    open: () => { isOpen.value = true; },
    close: () => { isOpen.value = false; },
    notifyCreated: () => { lastJobCreated.value++; },
  };
}
