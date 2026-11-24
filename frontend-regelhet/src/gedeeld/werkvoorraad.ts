// A submission in a work queue, in words, shared by the back office of a
// process (backoffice/WerkvoorraadView.vue) and the work queue of an
// organisation's front over all its processes (organisatie/), so both say
// the same of a submission.
import { longDate } from './datum';
import { indieningTitel } from './formulier';
import { artikelNaam, omschrijving, rolVan } from './keten';
import type { Casus, Gram, Indieningsvorm, Kroniek } from './types';

/** What a row says of a submission. */
export interface IndieningInWoorden {
  /** The submission, by its kind and the name its form records. */
  tekst: string;
  /** When it came in and under what; then what it waits for, or when it was decided. */
  regel: string;
  /** Whether its last decision was revoked, so it is to be decided again. */
  opnieuw: boolean;
  /** The day it waits since: the record to decide after, else the day it came in. */
  sinds: string;
}

/** What the words of a submission are made from; only `stuk` and `casus` are always known. */
export interface IndieningBron {
  stuk: Gram;
  casus: Casus | null;
  /** Its form, for the name the form records. */
  vorm?: Indieningsvorm;
  /** The executor's chronicle, to name the record to decide after. */
  kroniek?: Kroniek;
  /** The record to decide after, when its last decision was revoked. */
  na?: string;
  /** Whom it waits on, while a request about it is open. */
  wachtOp?: readonly string[];
  /** The day it was decided, once it was. */
  besloten?: string;
}

/**
 * A submission in words. Without the chronicle a revoked decision is still
 * marked (`opnieuw`), only without naming after what.
 */
export function indieningInWoorden({ stuk, casus, vorm, kroniek, na, wachtOp = [], besloten }: IndieningBron): IndieningInWoorden {
  const naGram = na ? kroniek?.grammen.find((g) => g.id === na) : undefined;
  const regel = [
    `Ontvangen ${longDate(stuk.recorded_on)} · onder ${artikelNaam(stuk.basis)}`,
    naGram && casus && kroniek
      ? `Opnieuw beslissen na ${omschrijving(naGram, casus, rolVan(naGram, kroniek.stand)).toLowerCase()} van ${longDate(naGram.recorded_on)}`
      : '',
    wachtOp.length ? `Wacht op ${wachtOp.join(' en ')}` : '',
    besloten ? `Besloten ${longDate(besloten)}` : '',
  ];
  return {
    tekst: casus ? indieningTitel(stuk, casus, vorm) : stuk.kind,
    regel: regel.filter(Boolean).join(' · '),
    opnieuw: !!na,
    sinds: naGram?.recorded_on ?? stuk.recorded_on,
  };
}
