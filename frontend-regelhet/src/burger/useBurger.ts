// What the citizen's screens load: the case, its forms, the dossier and who
// is logged in, through the API client of the public side only.
import { ref } from 'vue';
import { foutTekst } from '../gedeeld/http';
import type { Casus, Gram, Ik, Stand, VerzoekInDossier, Vervolgstand, Vormen } from '../gedeeld/types';
import { burgerApi } from './api';

export function useBurger() {
  const casus = ref<Casus | null>(null);
  const vormen = ref<Vormen | null>(null);
  const dossier = ref<Gram[]>([]);
  /** What the kern says of the dossier's records: what each is, what was revoked, what answers each submission. */
  const stand = ref<Stand>({ rollen: {}, herroepen: {}, antwoorden: {} });
  const vervolg = ref<Vervolgstand[]>([]);
  /** Per record the system made by an article of the policy, that article. */
  const doorHetSysteem = ref<Record<string, string>>({});
  /** The requests of the policy in the dossier, by the ids of their records. */
  const verzoeken = ref<VerzoekInDossier[]>([]);
  const wie = ref<Ik | null>(null);
  const fout = ref('');
  const laden = ref(true);

  async function laad() {
    laden.value = true;
    try {
      let d: Awaited<ReturnType<typeof burgerApi.dossier>>;
      [casus.value, vormen.value, d, wie.value] = await Promise.all([
        burgerApi.casus(),
        burgerApi.formulier(),
        burgerApi.dossier(),
        burgerApi.ik(),
      ]);
      dossier.value = d.grammen;
      stand.value = d.stand;
      vervolg.value = d.vervolg;
      doorHetSysteem.value = d.doorHetSysteem;
      verzoeken.value = d.verzoeken ?? [];
      fout.value = '';
    } catch (e) {
      fout.value = foutTekst(e);
    } finally {
      laden.value = false;
    }
  }

  return { casus, vormen, dossier, stand, vervolg, doorHetSysteem, verzoeken, wie, fout, laden, laad };
}
