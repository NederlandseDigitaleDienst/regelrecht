// How far the acts after a grant are, in words (`na_toekenning` in the case
// file): for the citizen the text the case gives the last act done, for an
// employee the act and its day. No act of a particular case is named here;
// the case gives the acts, their order and their texts.
import { longDate } from './datum';
import { feitNaam } from './feitnaam';
import { artikelNaam } from './keten';
import type { Casus, NaToekenning, Vervolgstand } from './types';

/** The last act of `stand` that is done, with its day, if one is. */
function laatste(stand: Vervolgstand): { feit: string; gedaan: string } | undefined {
  const gedaan = stand.stappen.filter((s): s is { feit: string; gedaan: string } => s.gedaan !== null);
  return gedaan.at(-1);
}

/** What a citizen reads of the grant `stand`, such as "Uw kaart ligt klaar.". */
export function burgerStand(stand: Vervolgstand, na: NaToekenning): string {
  const l = laatste(stand);
  if (!l) return na.stand;
  const tekst = na.handelingen.find((h) => h.feit === l.feit)?.stand ?? feitNaam(l.feit);
  return tekst.replaceAll('{datum}', longDate(l.gedaan));
}

/** How far the grant `stand` is for an employee: the last act and its day, or the grant. */
export function medewerkerStand(stand: Vervolgstand, na?: NaToekenning | null): string {
  const l = laatste(stand);
  if (l) {
    const automatisch = na?.handelingen.some((h) => h.feit === l.feit && h.door_het_systeem);
    return `${feitNaam(l.feit)} op ${longDate(l.gedaan)}${automatisch ? ', automatisch' : ''}`;
  }
  return stand.toegekend_op ? `Toegekend op ${longDate(stand.toegekend_op)}` : 'Toegekend';
}

/** The facts the case records after a grant, each in its turn, never on its own. */
export const vervolgFeiten = (casus: Casus | null): Set<string> =>
  new Set((casus?.na_toekenning?.handelingen ?? []).map((h) => h.feit));

/**
 * For a record the system made by an article of the policy, without a person
 * (`door_het_systeem` beside a chronicle or a dossier), a line that says so,
 * such as "Automatisch vastgelegd volgens Beleid ... artikel 7"; else nothing.
 */
export function doorHetSysteem(id: string, doorSysteem: Record<string, string> = {}): string {
  const artikel = doorSysteem[id];
  return artikel ? `Automatisch vastgelegd volgens ${artikelNaam(artikel)}` : '';
}
