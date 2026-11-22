// Which requests of the executor's policy a submission waits on, as the pane
// beside the work queue shows them (BehandelenPaneel.vue).
import type { Gram } from '../gedeeld/types';
import type { EigenVerzoek } from './api';

/** A request, and whether it was made for an earlier submission of the same person. */
export type VerzoekBij = EigenVerzoek & { eerder: boolean };

/**
 * The requests about the submission `indiening`: those made for it, and an
 * open one made for an earlier submission of the same person, which this one
 * waits on as well, since the policy asks nothing again while one is open. A
 * request names its submission by id; the same person is the same value in
 * every field the request and the submission both hold.
 */
export function verzoekenBij(indiening: Gram, verzoeken: EigenVerzoek[]): VerzoekBij[] {
  const velden = indiening.fields ?? {};
  return verzoeken.flatMap((v): VerzoekBij[] => {
    const waarden = v.verzoek.fields ?? {};
    if (Object.values(waarden).includes(indiening.id)) return [{ ...v, eerder: false }];
    const gedeeld = Object.keys(waarden).filter((k) => k in velden);
    const zelfde = gedeeld.length > 0 && gedeeld.every((k) => waarden[k] === velden[k]);
    return v.open && zelfde ? [{ ...v, eerder: true }] : [];
  });
}
