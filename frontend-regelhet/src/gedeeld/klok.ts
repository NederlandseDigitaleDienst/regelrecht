// The clock the trial band shows, shared by the shell and the screen that
// sets it. The shell loads it when the account or the route changes; the
// admin's clock screen puts the clock it just set here, so the band follows
// at once and does not wait for the next route.
import { ref } from 'vue';
import type { Klok } from './types';

export const huidigeKlok = ref<Klok | null>(null);
