// What the citizen's screens load: the case, its forms, the dossier and who
// is logged in, through the API client of the public side only.
import { ref } from 'vue';
import { foutTekst } from '../gedeeld/http';
import type { Casus, Gram, Ik, Stand, Vervolgstand, Vormen } from '../gedeeld/types';
import { burgerApi } from './api';

export function useBurger() {
  const casus = ref<Casus | null>(null);
  const vormen = ref<Vormen | null>(null);
  const dossier = ref<Gram[]>([]);
  /** What the kern says of the dossier's records: what each is, what was revoked, what answers each submission. */
  const stand = ref<Stand>({ rollen: {}, herroepen: {}, antwoorden: {} });
  const vervolg = ref<Vervolgstand[]>([]);
  const wie = ref<Ik | null>(null);
  const fout = ref('');
  const laden = ref(true);

  async function laad() {
    laden.value = true;
    try {
      let d: { grammen: Gram[]; stand: Stand; vervolg: Vervolgstand[] };
      [casus.value, vormen.value, d, wie.value] = await Promise.all([
        burgerApi.casus(),
        burgerApi.formulier(),
        burgerApi.dossier(),
        burgerApi.ik(),
      ]);
      dossier.value = d.grammen;
      stand.value = d.stand;
      vervolg.value = d.vervolg;
      fout.value = '';
    } catch (e) {
      fout.value = foutTekst(e);
    } finally {
      laden.value = false;
    }
  }

  return { casus, vormen, dossier, stand, vervolg, wie, fout, laden, laad };
}
