// One work queue over every process of the organisation: per process its
// case, its forms (for the name an application records), what waits for a
// decision of its executor, the requests of its policy (to say whom a
// submission waits on) and, only where a decision was revoked, its chronicle
// to say after what. The requests and the chronicle only add words: when one
// of them fails, the rows of the process stay, with less said. Each process is asked through
// the front (perProces.ts); its rows are made as the process's own back office
// makes them (gedeeld/werkvoorraad.ts). A process that fails stays out of the
// rows and is named apart, so the others stay on the screen.
import type { Casus, EigenVerzoek, Gram, Kroniek, Vormen, Werkvoorraad } from '../gedeeld/types';
import { indieningInWoorden, wachtOpBij } from '../gedeeld/werkvoorraad';
import type { IndieningInWoorden } from '../gedeeld/werkvoorraad';
import { openAdres, tegelNaam } from './api';
import type { Proces } from './api';
import { haalBijProces, perProces } from './perProces';
import type { ProcesAntwoord } from './perProces';

/** What one process gives the queue. */
export interface ProcesWerkvoorraad {
  casus: Casus;
  vormen: Vormen;
  werkvoorraad: Werkvoorraad;
  /** The requests of its policy; none when they could not be read. */
  verzoeken: EigenVerzoek[];
  /** The executor's chronicle, asked only when a decision was revoked. */
  kroniek?: Kroniek;
}

/** A row of the queue: the submission in the process's own words, with the process it is from. */
export interface OrganisatieRij extends IndieningInWoorden {
  stuk: Gram;
  proces: Proces;
  /** The executor's cell, for the address of the handling screen. */
  cel: string;
}

/** The queue: every row, and the processes that did not answer, with why. */
export interface Samengevoegd {
  rijen: OrganisatieRij[];
  onbereikbaar: { proces: Proces; fout: string }[];
}

/** Ask process `p` for its queue. */
export async function vraagWerkvoorraad(p: Proces): Promise<ProcesWerkvoorraad> {
  const [casus, vormen, werkvoorraad, verzoeken] = await Promise.all([
    haalBijProces<Casus>(p.id, 'casus'),
    haalBijProces<Vormen>(p.id, 'formulier'),
    haalBijProces<Werkvoorraad>(p.id, 'backoffice/werkvoorraad'),
    haalBijProces<{ verzoeken: EigenVerzoek[] }>(p.id, 'backoffice/verzoeken').then(
      (v) => v.verzoeken,
      () => [],
    ),
  ]);
  const kroniek = werkvoorraad.open.some((w) => w.na)
    ? await haalBijProces<Kroniek>(p.id, `backoffice/cellen/${encodeURIComponent(werkvoorraad.cel)}/kroniek`).catch(
        () => undefined,
      )
    : undefined;
  return { casus, vormen, werkvoorraad, verzoeken, kroniek };
}

/** Ask every process of `processen` for its queue. */
export const laadWerkvoorraad = (processen: readonly Proces[]): Promise<ProcesAntwoord<ProcesWerkvoorraad>[]> =>
  perProces(processen, vraagWerkvoorraad);

/**
 * The rows of every process that answered, the longest waiting first (by the
 * day a submission waits since: the day it came in, or the record to decide
 * after; on one day by the id of the submission, which grows with the time
 * it was made), and the processes that did not.
 */
export function samenvoegen(antwoorden: readonly ProcesAntwoord<ProcesWerkvoorraad>[]): Samengevoegd {
  const rijen: OrganisatieRij[] = [];
  const onbereikbaar: Samengevoegd['onbereikbaar'] = [];
  for (const a of antwoorden) {
    if ('fout' in a) {
      onbereikbaar.push(a);
      continue;
    }
    const { casus, vormen, werkvoorraad, verzoeken, kroniek } = a.antwoord;
    for (const w of werkvoorraad.open) {
      const woorden = indieningInWoorden({
        stuk: w.stuk,
        casus,
        vormen,
        kroniek,
        grammen: kroniek?.grammen,
        na: w.na,
        wachtOp: wachtOpBij(w.stuk, verzoeken),
      });
      rijen.push({ ...woorden, stuk: w.stuk, proces: a.proces, cel: werkvoorraad.cel });
    }
  }
  rijen.sort((x, y) => x.sinds.localeCompare(y.sinds) || x.stuk.id.localeCompare(y.stuk.id));
  return { rijen, onbereikbaar };
}

/** The value of the filter that shows every process. */
export const ALLE = 'alle';

/** The rows of the process `id`, or every row for {@link ALLE}. */
export const filter = (rijen: readonly OrganisatieRij[], id: string): OrganisatieRij[] =>
  id === ALLE ? [...rijen] : rijen.filter((r) => r.proces.id === id);

/** The address that opens the row's submission in the handling screen of its process, through the hand-off. */
export const behandelAdres = (r: OrganisatieRij): string =>
  openAdres(r.proces.id, `/backoffice/behandelen/${encodeURIComponent(r.cel)}/${encodeURIComponent(r.stuk.id)}`);

/** The process's name on a row and in the filter. */
export const procesNaam = tegelNaam;
