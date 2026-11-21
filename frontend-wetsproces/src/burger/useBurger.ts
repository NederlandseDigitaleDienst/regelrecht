// What the citizen's screens load: the case, its forms, the dossier and who
// is logged in, through the API client of the public side only.
import { ref } from 'vue';
import { foutTekst } from '../gedeeld/http';
import type { Casus, Gram, Ik, Vormen } from '../gedeeld/types';
import { burgerApi } from './api';

export function useBurger() {
  const casus = ref<Casus | null>(null);
  const vormen = ref<Vormen | null>(null);
  const dossier = ref<Gram[]>([]);
  const wie = ref<Ik | null>(null);
  const fout = ref('');
  const laden = ref(true);

  async function laad() {
    laden.value = true;
    try {
      [casus.value, vormen.value, dossier.value, wie.value] = await Promise.all([
        burgerApi.casus(),
        burgerApi.formulier(),
        burgerApi.dossier(),
        burgerApi.ik(),
      ]);
      fout.value = '';
    } catch (e) {
      fout.value = foutTekst(e);
    } finally {
      laden.value = false;
    }
  }

  return { casus, vormen, dossier, wie, fout, laden, laad };
}
