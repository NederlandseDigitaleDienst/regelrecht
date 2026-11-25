// The address of the sheet that records a fact. It is a hash over whatever
// screen opened it, so a link to it opens the sheet on that screen, a reload
// keeps it open, and the browser's back button closes it, as with the
// Besluiten sheet. `#vastleggen/<cel>/<fact>` records a fact of its own;
// `#vastleggen/<cel>/<fact>/<record>` one beside a record, or in answer to a
// request. `#vastleggen-herkomst/…` is the page of the same sheet that says
// where the fields come from. A fact goes by the last part of its name
// (`betaling_ontvangen`); the screen looks it up among the facts it offers.
import { computed } from 'vue';
import type { ComputedRef } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { Handeling } from '../gedeeld/types';

export interface Vastlegadres {
  cel: string;
  feit: string;
  /** The record the fact is recorded beside, or the request it answers. */
  bij?: string;
  /** The page that says where the fields come from. */
  herkomst: boolean;
}

const OPEN = '#vastleggen/';
const HERKOMST = '#vastleggen-herkomst/';

const feitDeel = (feit: string): string => feit.split('#').at(-1) ?? feit;

/** The hash of the sheet for `adres`, on its form or (`herkomst`) on the page of where the fields come from. */
export function vastlegHash(adres: Omit<Vastlegadres, 'herkomst'>, herkomst = false): string {
  const delen = [adres.cel, feitDeel(adres.feit), ...(adres.bij ? [adres.bij] : [])];
  return (herkomst ? HERKOMST : OPEN) + delen.map(encodeURIComponent).join('/');
}

/** The sheet a hash opens, or none when it is not one of these. */
export function leesVastlegHash(hash: string): Vastlegadres | null {
  const herkomst = hash.startsWith(HERKOMST);
  if (!herkomst && !hash.startsWith(OPEN)) return null;
  try {
    const [cel, feit, bij, ...rest] = hash.slice((herkomst ? HERKOMST : OPEN).length).split('/').map(decodeURIComponent);
    if (!cel || !feit || rest.length) return null;
    return { cel, feit, ...(bij ? { bij } : {}), herkomst };
  } catch {
    return null;
  }
}

/** Whether `h` is the fact an address names. */
export const isFeit = (h: Handeling, adres: Vastlegadres): boolean => feitDeel(h.feit) === adres.feit;

interface Keuze {
  cel: string;
  handeling: Handeling;
  verwijst_naar?: string;
  verzoek?: string;
}

/**
 * The fact the sheet records, read from the address: `zoek` finds it among
 * what the screen has loaded, so the sheet opens once that is there. `kies`
 * opens the sheet on a fact, `sluit` closes it, both on the screen it is on.
 */
export function useVastlegKeuze<K extends Keuze>(
  zoek: (adres: Vastlegadres) => K | null | undefined,
): { keuze: ComputedRef<K | null>; kies: (k: K) => void; sluit: () => void } {
  const route = useRoute();
  const router = useRouter();
  const adres = computed(() => leesVastlegHash(route.hash));
  const keuze = computed(() => (adres.value ? (zoek(adres.value) ?? null) : null));
  const kies = (k: K) =>
    void router.push({
      path: route.path,
      query: route.query,
      hash: vastlegHash({ cel: k.cel, feit: k.handeling.feit, bij: k.verwijst_naar ?? k.verzoek }),
    });
  const sluit = () => {
    if (adres.value) void router.push({ path: route.path, query: route.query });
  };
  return { keuze, kies, sluit };
}
