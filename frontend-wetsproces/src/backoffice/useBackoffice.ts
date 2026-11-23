// What the back office screens load: the case, its forms, who is logged in
// and the chronicle of each cell the employee may read.
import { ref } from 'vue';
import { foutTekst } from '../gedeeld/http';
import type { Casus, Ik, Vormen } from '../gedeeld/types';
import { backofficeApi } from './api';
import type { Kroniek } from './api';

export function useBackoffice() {
  const casus = ref<Casus | null>(null);
  const vormen = ref<Vormen | null>(null);
  const wie = ref<Ik | null>(null);
  const kronieken = ref<Kroniek[]>([]);
  const fout = ref('');
  const laden = ref(true);

  /** Load it all; `stil` leaves `laden` alone, so what shows stays while it loads again. */
  async function laad(stil = false) {
    if (!stil) laden.value = true;
    try {
      const [c, v, w] = await Promise.all([backofficeApi.casus(), backofficeApi.formulier(), backofficeApi.ik()]);
      casus.value = c;
      vormen.value = v;
      wie.value = w;
      kronieken.value = await Promise.all(w.cellen.map((cel) => backofficeApi.kroniek(cel.cel)));
      fout.value = '';
    } catch (e) {
      fout.value = foutTekst(e);
    } finally {
      laden.value = false;
    }
  }

  return { casus, vormen, wie, kronieken, fout, laden, laad };
}
