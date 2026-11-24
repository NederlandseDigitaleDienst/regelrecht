// The timeline of an application in words, for the citizen and the
// employee alike (TijdlijnLijst.vue). What happened and what comes next is
// the server's (`Tijdlijn`); this only names it, with the words the records,
// the case and the UI spec give: a record by what the kern says it is
// (`omschrijving`), a fact by its name, a deadline by the label of its field.
// A next step goes by the task the case names (`taak`): a fact names what is
// recorded once the step is taken, not the step.
// No step of a particular law is named here.
import { longDate } from './datum';
import { feitNaam } from './feitnaam';
import { kaalSchermveld } from './formulier';
import { artikelNaam, besluitStand, celNaam, omschrijving, rolVan } from './keten';
import type { AanZet, Casus, Gram, Tijdlijn, Veldtekst, VerzoekInDossier, VolgendeStap } from './types';
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

/**
 * A stage of a request of the policy after the request itself: done, or the
 * one it waits on. The answer is a step of the process, as the request is; a
 * receipt only records when the other side had it, and stands under the step.
 */
export interface Verzoekstap {
  tekst: string;
  regel: string;
  gedaan: boolean;
  level: 'major' | 'minor';
}

/** A request of the policy that another party answers, with its stages under it. */
export interface Verzoekregel extends Regel {
  verzoek: {
    /** Who asked and who was asked, in words. */
    van: string;
    aan: string;
    /** The article it was asked by, in words. */
    artikel: string;
    /** Whether the system recorded it by itself, by that article. */
    automatisch: boolean;
    stappen: Verzoekstap[];
    /** The answer, once given: what it holds and the request does not. */
    antwoord: Record<string, unknown> | null;
  };
}

/** A record of the timeline with its day and the cell that recorded or received it. */
const wanneerWie = (g: Gram, casus: Casus) =>
  `${longDate(g.recorded_on)} · ${celNaam(g.received?.from ?? g.recording_actor, casus)}`;

/**
 * `rijen` with each request of `verzoeken` as a row that carries the records
 * belonging to it as its stages: the other party's receipt, its answer and
 * the receipt of that, and the next step that waits on the answer. Those are
 * steps of one exchange, so they stand under the request rather than as rows
 * of their own. Who was asked comes from their receipt, or, before it, from
 * the next step that waits on them. Of the answer only what the request does
 * not already carry is kept: the application, the applicant and the day of
 * asking are on screen already.
 */
export function metVerzoeken(rijen: Regel[], verzoeken: VerzoekInDossier[], t: Tijdlijn, casus: Casus, kant: Kant): (Regel | Verzoekregel)[] {
  const vind = (id: string | null) => (id ? t.grammen.find((g) => g.id === id) : undefined);
  const binnen = new Set(verzoeken.flatMap((v) => [v.ontvangen, v.antwoord, v.antwoord_ontvangen]).filter(Boolean));
  return rijen
    .filter((r) => !binnen.has(r.gram.id))
    .map((r): Regel | Verzoekregel => {
      const v = verzoeken.find((x) => x.verzoek === r.gram.id);
      if (!v) return r;
      const [ontvangen, antwoord, terug] = [vind(v.ontvangen), vind(v.antwoord), vind(v.antwoord_ontvangen)];
      const wacht = t.volgende.find((s) => s.soort === 'antwoord' && s.op === v.verzoek);
      const gevraagd = ontvangen?.recording_actor ?? (wacht && 'cel' in wacht ? wacht.cel : undefined);
      const van = celNaam(r.gram.recording_actor, casus);
      const aan = gevraagd ? celNaam(gevraagd, casus) : '';
      const stap = (g: Gram | undefined, tekst: string, level: Verzoekstap['level']): Verzoekstap[] =>
        g
          ? [{ tekst: noem(g, t, casus), regel: wanneerWie(g, casus), gedaan: true, level }]
          : tekst
            ? [{ tekst, regel: '', gedaan: false, level }]
            : [];
      const stappen = [
        ...stap(ontvangen, '', 'minor'),
        ...stap(antwoord, wacht && wacht.soort === 'antwoord' ? (wacht.taak ?? feitNaam(wacht.feit)) : '', 'major'),
        ...stap(terug, '', 'minor'),
      ];
      // The stage it waits on says who is to act.
      const open = stappen.find((s) => !s.gedaan);
      if (open && wacht) open.regel = wie(wacht, casus, kant);
      const eigen = new Set(Object.keys(r.gram.fields ?? {}));
      return {
        ...r,
        regel: `${longDate(r.gram.recorded_on)} · ${aan ? `${van} aan ${aan}` : van}`,
        verzoek: {
          van,
          aan,
          artikel: artikelNaam(r.gram.basis),
          automatisch: !!t.door_het_systeem[r.gram.id],
          stappen,
          antwoord: antwoord ? Object.fromEntries(Object.entries(antwoord.fields ?? {}).filter(([k]) => !eigen.has(k))) : null,
        },
      };
    });
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
      return { tekst: s.taak ?? feitNaam(s.feit), regel: `${aan} · Op ${stukTekst(s.op, t, casus)}` };
    case 'handeling': {
      const knop = casus.na_toekenning?.handelingen.find((h) => h.feit === s.feit)?.knop;
      const tekst = (kant === 'medewerker' && knop) || s.taak || feitNaam(s.feit);
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
