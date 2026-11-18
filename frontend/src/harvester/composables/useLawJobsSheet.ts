import { ref } from 'vue';

const isOpen = ref(false);
const lawId = ref('');

export function useLawJobsSheet() {
  return {
    isOpen,
    lawId,
    open: (id: string) => { lawId.value = id; isOpen.value = true; },
    close: () => { isOpen.value = false; },
  };
}
