// The timeline of an application in words, for the citizen and the
// employee alike (TijdlijnLijst.vue). What happened and what comes next is
// the server's (`Tijdlijn`); this only names it, with the words the records,
// the case and the UI spec give: a record by what the kern says it is
// (`omschrijving`), a fact by its name, a deadline by the label of its field.
// No step of a particular law is named here.
import { longDate } from './datum';
import { feitNaam } from './feitnaam';
import { kaalSchermveld } from './formulier';
import { artikelNaam, besluitStand, celNaam, omschrijving, rolVan } from './keten';
import type { AanZet, Casus, Gram, Tijdlijn, Veldtekst, VolgendeStap } from './types';
import { doorHetSysteem } from './vervolg';

/** Whose screen the timeline is on: the citizen reads "u", the employee who acts where. */
export type Kant = 'burger' | 'medewerker';

/** A row of the timeline. */
export interface Regel {
  gram: Gram;
  tekst: string;
  regel: string;
  stand: { tekst: string; kleur: string } | null;
}

/** A next step, in words. */
export interface Stap {
  tekst: string;
  regel: string;
}

/** A record of the timeline in words, by what the server says it is. */
const noem = (g: Gram, t: Tijdlijn, casus: Casus) => omschrijving(g, casus, rolVan(g, t.stand));

/** The rows of the timeline, in the order the server gives them. */
export function regels(t: Tijdlijn, casus: Casus): Regel[] {
  return t.grammen.map((g) => ({
    gram: g,
    tekst: noem(g, t, casus),
    regel: [longDate(g.recorded_on), celNaam(g.received?.from ?? g.recording_actor, casus), doorHetSysteem(g.id, t.door_het_systeem)]
      .filter(Boolean)
      .join(' · '),
    stand: besluitStand(g, t.stand, t.grammen, casus),
  }));
}

/** Who is to act, in words, on the screen of `kant`. */
function wie(stap: VolgendeStap, casus: Casus, kant: Kant): string {
  const cel = 'cel' in stap ? celNaam(stap.cel, casus) : '';
  const woorden: Record<AanZet, string> = {
    uitvoerder: cel,
    systeem: 'het systeem',
    externe_partij: kant === 'medewerker' ? `${cel}, in te voeren in het beheerpaneel` : cel,
    indiener: kant === 'burger' ? 'u' : 'de indiener',
  };
  return `Aan zet: ${woorden[stap.wie]}`;
}

/** The record `id` of the timeline in words with its day, lower case, or a neutral word when it is not there. */
function stukTekst(id: string, t: Tijdlijn, casus: Casus): string {
  const g = t.grammen.find((x) => x.id === id);
  return g ? `${noem(g, t, casus).toLowerCase()} van ${longDate(g.recorded_on)}` : 'een eerder stuk';
}

/**
 * A next step in words: what is to happen and who is to act, with the day
 * it must happen by where the server gives one. `teksten` names the field a
 * deadline is in; `knop` the act after a grant as the case words it for the
 * one who records it.
 */
export function stap(s: VolgendeStap, t: Tijdlijn, casus: Casus, kant: Kant, teksten?: Record<string, Veldtekst>): Stap {
  const aan = wie(s, casus, kant);
  switch (s.soort) {
    case 'besluit':
      return {
        tekst: `${s.na ? 'Opnieuw beslissen' : 'Beslissen'} op ${stukTekst(s.op, t, casus)}`,
        regel: s.na ? `${aan} · Na ${stukTekst(s.na, t, casus)}` : aan,
      };
    case 'antwoord':
      return { tekst: feitNaam(s.feit), regel: `${aan} · Op ${stukTekst(s.op, t, casus)}` };
    case 'handeling': {
      const knop = casus.na_toekenning?.handelingen.find((h) => h.feit === s.feit)?.knop;
      const tekst = kant === 'medewerker' && knop ? knop : feitNaam(s.feit);
      return { tekst, regel: s.artikel ? `${aan}, volgens ${artikelNaam(s.artikel)}` : aan };
    }
    case 'toezending':
      return { tekst: `Afschrift van ${stukTekst(s.op, t, casus)}`, regel: aan };
    case 'termijn':
      return {
        tekst: `${kaalSchermveld(s.veld, teksten).label}: ${longDate(s.tot)}`,
        regel: `${aan} · Bij ${stukTekst(s.op, t, casus)}`,
      };
  }
}
