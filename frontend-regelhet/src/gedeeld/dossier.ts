// A citizen's dossier summed up as their applications: a row per
// application, with what was applied for, when and by which cell, how it
// stands (`stand`: what answers it now), and how far the acts after its grant
// are, in the words the case gives them (`na_toekenning`). The citizen's own
// list in a process (burger/DossierView.vue) and the list over every process
// of an organisation (organisatie/MijnAanvragenView.vue) make their rows with
// it, so both say the same of the same application.
import { antwoordOp, herroeping, isIndiening, ketens, omschrijving, onderVerzoeken, rolVan, toegewezen } from './keten';
import type { Knoop } from './keten';
import type { Casus, Gram, Stand, VerzoekInDossier, Vervolgaanvraag, Vervolgstand } from './types';
import { burgerStand } from './vervolg';

/** The dossier as the server sends it (`GET /api/dossier`). */
export interface DossierAntwoord {
  grammen: Gram[];
  stand: Stand;
  vervolg?: Vervolgstand[];
  door_het_systeem?: Record<string, string>;
  verzoeken?: VerzoekInDossier[];
}

/**
 * The records about the citizen, what the kern says of them (`stand`), how
 * far the acts after each of their grants are, per record the system made by
 * an article of the policy that article, and per request of the policy the
 * ids of its records in the dossier.
 */
export interface Dossier {
  grammen: Gram[];
  stand: Stand;
  vervolg: Vervolgstand[];
  doorHetSysteem: Record<string, string>;
  verzoeken: VerzoekInDossier[];
}

/** The dossier `d` the server sent, with what it may leave out filled in. */
export const leesDossier = (d: DossierAntwoord): Dossier => ({
  grammen: d.grammen,
  stand: d.stand,
  vervolg: d.vervolg ?? [],
  doorHetSysteem: d.door_het_systeem ?? {},
  verzoeken: d.verzoeken ?? [],
});

/** How an application stands, as a word and the colour of the dot beside it. */
export interface AanvraagStand {
  tekst: string;
  kleur: string;
}

/** An application of the dossier, as a row of a list. */
export interface Aanvraag {
  gram: Gram;
  /** What it asks, in words. */
  tekst: string;
  /** How far the acts after its grant are, or empty. */
  vervolg: string;
  stand: AanvraagStand | null;
  /** What the citizen can apply for about its grant, each with the record it is about. */
  aanvragen: (Vervolgaanvraag & { op: string })[];
}

/** `k` and every record under it. */
const alleKnopen = (k: Knoop): Knoop[] => [k, ...k.onder.flatMap(alleKnopen)];

/**
 * Every application in the dossier `d` of the case `casus`, as one row each.
 * What followed on it (an investigation, the decision, an objection) belongs
 * to its row; an application about its grant, such as a duplicate, is a row
 * of its own.
 */
export function aanvragen(casus: Casus, d: Dossier): Aanvraag[] {
  // The articles of an application: the case's own, and those it offers about a grant.
  const artikelen = new Set([casus.artikel, ...(casus.na_toekenning?.aanvragen ?? []).map((a) => a.artikel)]);
  const isAanvraag = (g: Gram) => isIndiening(g, d.stand) && artikelen.has(g.basis);
  /** The records of the application `k`: it and what follows on it, without another application under it. */
  const vanAanvraag = (k: Knoop): Gram[] => [k.gram, ...k.onder.filter((o) => !isAanvraag(o.gram)).flatMap(vanAanvraag)];
  return ketens(d.grammen, onderVerzoeken(d.verzoeken))
    .flatMap(alleKnopen)
    .filter((k) => isAanvraag(k.gram))
    .map((k) => {
      const stukken = vanAanvraag(k);
      return {
        gram: k.gram,
        tekst: aanvraagNaam(k.gram, casus, d),
        vervolg: stukken.map((g) => vervolgTekst(g, casus, d)).filter(Boolean).at(-1) ?? '',
        stand: aanvraagStand(k.gram, casus, d),
        aanvragen: stukken.flatMap((g) => aanvragenBij(g.id, casus, d).map((a) => ({ ...a, op: g.id }))),
      };
    });
}

/** What an application asks, in words: the case's own after its title, one about a grant by the title of its form. */
function aanvraagNaam(g: Gram, casus: Casus, d: Dossier): string {
  const over = casus.na_toekenning?.aanvragen?.find((a) => a.artikel === g.basis);
  if (over) return over.titel;
  const soort = omschrijving(g, casus, rolVan(g, d.stand));
  return casus.titel ? `${soort} ${casus.titel.toLowerCase()}` : soort;
}

/** How a submission of the dossier stands, by what answers it now (`stand`), an objection as well as an application. */
export function aanvraagStand(g: Gram, casus: Casus, d: Dossier): AanvraagStand | null {
  const a = antwoordOp(g.id, d.stand);
  if (!a) return null;
  // Revoked with a new decision to follow, after a ruling: it waits again.
  if (a.soort === 'open' || (a.soort === 'vervallen' && a.na)) return { tekst: 'In behandeling', kleur: 'accent' };
  if (a.soort === 'vervallen') {
    return { tekst: herroeping(a.besluit, d.stand, d.grammen)?.woord ?? 'Herroepen', kleur: 'warning' };
  }
  const t = toegewezen(a.soort === 'in_de_plaats' ? a.nieuw : (d.grammen.find((x) => x.id === a.besluit) ?? {}), casus);
  if (t === undefined) return null;
  return t ? { tekst: 'Toegewezen', kleur: 'success' } : { tekst: 'Afgewezen', kleur: 'critical' };
}

/** How far the acts after the grant `g` are, in the citizen's words, if `g` granted and the case names such acts. */
function vervolgTekst(g: Gram, casus: Casus, d: Dossier): string {
  const na = casus.na_toekenning;
  if (!na) return '';
  const s = d.vervolg.find((v) => v.toekenning === g.id);
  if (s) return burgerStand(s, na);
  // A grant in an aggregate whose grant is now the decision on an
  // application about a grant, such as a duplicate: that one took its place.
  // Not a grant a decision on objection replaced: that is a revocation.
  const vervangers = new Set((na.aanvragen ?? []).map((a) => a.artikel));
  const vervangen = d.vervolg.some((v) => {
    const nu = d.grammen.find((x) => x.id === v.toekenning);
    return v.toekenning !== g.id && !!nu && vervangers.has(nu.basis) && hoortBij(g, v.besluit, d.grammen);
  });
  return vervangen && toegewezen(g, casus) === true ? (na.vervallen ?? '') : '';
}

/** Whether `g` is the record `wortel` or refers to it, however indirectly. */
function hoortBij(g: Gram, wortel: string, grammen: Gram[]): boolean {
  const gezien = new Set<string>();
  let huidig: Gram | undefined = g;
  while (huidig && !gezien.has(huidig.id)) {
    if (huidig.id === wortel) return true;
    gezien.add(huidig.id);
    const naar: string | null | undefined = huidig.refers_to;
    huidig = naar ? grammen.find((x) => x.id === naar) : undefined;
  }
  return false;
}

/**
 * What the citizen can apply for about the grant `id`, as the case offers it
 * (`na_toekenning.aanvragen`): only once every act after it is done, and not
 * while such an application about it waits for a decision.
 */
function aanvragenBij(id: string, casus: Casus, d: Dossier): Vervolgaanvraag[] {
  const na = casus.na_toekenning;
  const s = d.vervolg.find((v) => v.toekenning === id);
  if (!na?.aanvragen || !s || s.volgende) return [];
  return na.aanvragen.filter(
    (a) => !d.grammen.some((g) => g.basis === a.artikel && g.refers_to === id && !antwoordOp(g.id, d.stand)),
  );
}
