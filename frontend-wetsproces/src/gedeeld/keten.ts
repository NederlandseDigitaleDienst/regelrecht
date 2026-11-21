// What a list of records says about the process: which record is a
// submission and which a decision, what refers to what, and what still waits
// for a decision. Read from the records and the case alone: a submission is
// a record of a kind the case's regelwerken establish as one
// (`casus.indieningen`), a decision is a record that answers to one, and no
// step of a particular law is named here.
import {
  artikelNaam as gedeeldArtikelNaam,
  hoofdletter,
  leesbaar,
  toon,
  wetNaam,
} from '@regelrecht/frontend-shared/chronolex/waarden';
import { longDate } from './datum';
import type { Casus, Gram } from './types';

/** The kind of record a cell keeps of a fact it recorded. */
const VASTLEGGING = 'EXECUTOGRAM';

/** Whether `g` is a submission: of a kind the case establishes as one. */
export const isIndiening = (g: Gram, casus: Casus): boolean => casus.indieningen.some((i) => i.soort === g.kind);

/** Whether `g` is a fact a cell recorded. */
export const isVastlegging = (g: Gram): boolean => g.kind === VASTLEGGING;

/** Whether `g` is a decision: neither a submission nor a recorded fact. */
export const isBesluit = (g: Gram, casus: Casus): boolean => !isIndiening(g, casus) && !isVastlegging(g);

/** The records in `grammen` that refer to `id`, in chronicle order. */
export const verwijzendNaar = (id: string, grammen: Gram[]): Gram[] => grammen.filter((g) => g.refers_to === id);

/** The decisions in `grammen` on the submission `id`, the latest last. */
export const besluitenOp = (id: string, grammen: Gram[], casus: Casus): Gram[] =>
  verwijzendNaar(id, grammen).filter((g) => isBesluit(g, casus));

/** A record with what refers to it, and so on down. */
export interface Knoop {
  gram: Gram;
  onder: Knoop[];
}


/**
 * The records of `grammen` as chains: each record that hangs under none of
 * them is the top of one, with what hangs under it underneath, in the order
 * the records came: application, decision, objection, decision on it,
 * appeal, ruling. A record found in two chronicles (a fact one cell sends
 * another) counts once.
 */
export function ketens(grammen: Gram[]): Knoop[] {
  const uniek = [...new Map(grammen.map((g) => [g.id, g])).values()];
  const ids = new Set(uniek.map((g) => g.id));
  // What `g` hangs under: what it received from another cell when that is
  // in the list, such as the decision an appeal to the court contests or the
  // ruling whose copy the municipality received; else the record it refers to.
  const ouder = (g: Gram): string | undefined =>
    (g.received && ids.has(g.received.id) ? g.received.id : undefined) ?? g.refers_to ?? undefined;
  const knoop = (g: Gram): Knoop => ({ gram: g, onder: uniek.filter((x) => ouder(x) === g.id).map(knoop) });
  return uniek.filter((g) => {
    const o = ouder(g);
    return !o || !ids.has(o);
  }).map(knoop);
}

/** The submissions in the chronicle `grammen` on which no decision was taken yet. */
export const openIndieningen = (grammen: Gram[], casus: Casus): Gram[] =>
  grammen.filter((g) => isIndiening(g, casus) && besluitenOp(g.id, grammen, casus).length === 0);

/**
 * The submissions a person can make against the decision `besluit`, in the
 * order the Awb sets: against a decision on an application, what the same
 * cell decides on (an objection); against the decision of the case's own
 * cell on such a submission, what another cell decides on (an appeal to a
 * court). Against anything else, such as the court's ruling, nothing: a
 * further step is not in any case yet. An application refers to no
 * decision, so it is never offered.
 */
export function indieningenTegen(besluit: Gram, casus: Casus): Casus['indieningen'] {
  // An application of any article is of the kind of the case's own application.
  // A case that names no submission of its own article gives no way to tell an
  // application from the rest, so nothing is offered.
  const aanvraag = casus.indieningen.find((i) => i.artikel === casus.artikel)?.soort;
  if (!aanvraag) return [];
  const tegenBesluit = casus.indieningen.filter((i) => i.soort !== aanvraag);
  const hier = tegenBesluit.filter((i) => i.cel === besluit.recording_actor);
  const opAanvraag = casus.indieningen.some((i) => i.soort === aanvraag && i.besluit === besluit.basis);
  if (opAanvraag) return hier;
  const opBezwaar = besluit.recording_actor === casus.cel && hier.some((i) => i.besluit === besluit.basis);
  return opBezwaar ? tegenBesluit.filter((i) => i.cel !== besluit.recording_actor) : [];
}

/** Whether `v` is the id of a record, which the chronicles give out as a UUID. */
const isRecordId = (v: string): boolean => /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v);

/**
 * A value of a record in words: a date as a screen writes it, a decision a
 * record holds (such as the contested decision of an objection) by the
 * article it was taken under, a reference to another record by what
 * `noem` calls it (or as an earlier record when it cannot), and anything
 * else as `toon` gives it. An id is never shown as it is.
 */
export function waardeTekst(v: unknown, noem?: (id: string) => string | undefined): string {
  if (typeof v === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(v)) return longDate(v);
  if (typeof v === 'string' && isRecordId(v)) return noem?.(v) ?? 'een eerder vastgelegd stuk';
  if (v && typeof v === 'object' && 'basis' in v && typeof v.basis === 'string') return `een besluit onder ${artikelNaam(v.basis)}`;
  return toon(v);
}

/** `<regelwerk>#<article>` as a screen writes it, such as `Awb artikel 4:2`. */
export function artikelNaam(basis: string): string {
  const [wet = basis, artikel] = basis.split('#');
  return artikel ? gedeeldArtikelNaam(wet, artikel) : wetNaam(wet);
}

/** The name of the cell `cel` in the case, or its id. */
export function celNaam(cel: string, casus: Casus): string {
  if (cel === casus.cel) return casus.cel_naam;
  return casus.andere_cellen.find((a) => a.cel === cel)?.cel_naam ?? cel;
}

/** Whether the decision `g` grants what was asked, by the case's outcome; undefined when it does not say. */
export function toegewezen(g: Gram, casus: Casus): boolean | undefined {
  const v = casus.uitkomst ? g.outputs?.[casus.uitkomst] : undefined;
  return typeof v === 'boolean' ? v : undefined;
}

/** What a record is, in words: its kind of submission, the fact it records, or a decision. */
export function omschrijving(g: Gram, casus: Casus): string {
  if (isVastlegging(g)) return hoofdletter(leesbaar(g.fact ?? 'vastlegging'));
  if (isIndiening(g, casus)) return hoofdletter(leesbaar(g.kind.toLowerCase()));
  // A decision of another cell goes by its own name, such as a court's ruling.
  return g.recording_actor === casus.cel ? `Besluit (${leesbaar(g.kind.toLowerCase())})` : besluitNaam(g, casus);
}

/**
 * The output of a decision that holds the last day to object to it or
 * appeal against it, as the Awb in the corpus derives it (articles 6:7 and
 * 6:8). The objection takes the same field over from the decision it
 * contests. A decision without it gives no deadline to show.
 */
const TERMIJN_EINDE = 'termijn_einddatum';

/** The deadline of decision `besluit` on day `vandaag`: its last day, and whether that has passed. */
export function termijn(besluit: Gram, vandaag: string): { einde: string; voorbij: boolean } | null {
  const einde = besluit.outputs?.[TERMIJN_EINDE];
  if (typeof einde !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(einde)) return null;
  // ISO days compare as text.
  return { einde, voorbij: vandaag > einde };
}

/**
 * What a decision is called on its own screen: a decision of the case's own
 * cell is a `Besluit`; one of another cell goes by its legal character as
 * the regelwerk names it, so a court's ruling is an `Uitspraak`.
 */
export function besluitNaam(g: Gram, casus: Casus): string {
  return g.recording_actor === casus.cel ? 'Besluit' : hoofdletter(leesbaar(g.kind.toLowerCase()));
}

/** The records of the chain under `k`, the top first, each with the record it answers to. */
export function plat(k: Knoop, boven?: Gram): { gram: Gram; boven?: Gram }[] {
  return [{ gram: k.gram, boven }, ...k.onder.flatMap((o) => plat(o, k.gram))];
}
