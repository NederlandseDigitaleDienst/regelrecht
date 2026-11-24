// "Mijn aanvragen" over every process of an organisation: the front asks each
// process for its case and the citizen's dossier (through its relay, with a
// statement of who is logged in), makes the rows as the process's own list
// does (gedeeld/dossier.ts), and puts them in one list, newest first. Each row
// opens the application in its process through the front's hand-off.
//
// A process that fails or does not answer drops out of the list and is named
// apart, so the citizen knows something is missing. A process that refuses
// the dossier (403) knows citizens by another identifier than the one this
// login carries, such as a KvK number where the login gave a BSN: the statement
// passes every claim of the login on, and each process reads its own
// (`REGELHET_SUBJECT_CLAIM`). Nothing there is the citizen's, so that process
// is left out without a notice.
import { ApiError } from '@regelrecht/frontend-shared';
import { aanvragen, leesDossier } from '../gedeeld/dossier';
import type { Aanvraag, AanvraagStand, DossierAntwoord } from '../gedeeld/dossier';
import type { Casus } from '../gedeeld/types';
import { openAdres } from './api';
import type { Proces } from './api';
import { beantwoord, haalBijProces, perProces } from './perProces';
import type { ProcesAntwoord } from './perProces';

/** An application of the citizen in one of the processes, as a row of the list. */
export interface MijnAanvraag {
  proces: Proces;
  /** The record of the application in its process. */
  id: string;
  /** What it asks, in words. */
  tekst: string;
  /** The day it was recorded, `YYYY-MM-DD`. */
  datum: string;
  /** How far the acts after its grant are, or empty. */
  vervolg: string;
  stand: AanvraagStand | null;
  /** Where the row leads: the application in its process, through the hand-off. */
  href: string;
}

/** The citizen's applications over every process, and the processes that gave none because they failed. */
export interface MijnAanvragen {
  aanvragen: MijnAanvraag[];
  nietBereikbaar: Proces[];
}

/** The applications of the citizen in `proces`, or null when that process knows citizens by another identifier. */
async function vanProces(proces: Proces): Promise<Aanvraag[] | null> {
  try {
    const [casus, dossier] = await Promise.all([
      haalBijProces<Casus>(proces.id, 'casus'),
      haalBijProces<DossierAntwoord>(proces.id, 'dossier'),
    ]);
    return aanvragen(casus, leesDossier(dossier));
  } catch (e) {
    if (e instanceof ApiError && e.status === 403) return null;
    throw e;
  }
}

/** Where a row leads: the page of the application in its process. */
export const aanvraagAdres = (proces: Proces, id: string): string =>
  openAdres(proces.id, `/burger/dossier/${encodeURIComponent(id)}`);

/**
 * The answers of the processes as one list, newest first and, on the same
 * day, in the order of the processes; a process that failed is named in
 * `nietBereikbaar`, one that does not know this citizen is left out.
 */
export function samenvoegen(antwoorden: ProcesAntwoord<Aanvraag[] | null>[]): MijnAanvragen {
  const rijen = antwoorden.filter(beantwoord).flatMap(({ proces, antwoord }) =>
    (antwoord ?? []).map((a) => ({
      proces,
      id: a.gram.id,
      tekst: a.tekst,
      datum: a.gram.recorded_on,
      vervolg: a.vervolg,
      stand: a.stand,
      href: aanvraagAdres(proces, a.gram.id),
    })),
  );
  // Array.prototype.sort is stable, so a day keeps the order of the processes.
  rijen.sort((a, b) => b.datum.localeCompare(a.datum));
  return { aanvragen: rijen, nietBereikbaar: antwoorden.filter((a) => !beantwoord(a)).map((a) => a.proces) };
}

/** The citizen's applications in every process of `processen`. */
export async function mijnAanvragen(processen: readonly Proces[]): Promise<MijnAanvragen> {
  return samenvoegen(await perProces(processen, vanProces));
}
