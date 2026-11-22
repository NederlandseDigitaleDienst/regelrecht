// What a form shows, from what the server derives from the law (`velden`)
// and what the UI spec of the case says of each field (`systeem`,
// `verborgen`, `teksten`, `volgorde`). Nothing here names a field of a case:
// without a UI spec every field the law asks is on the form, in the order
// of the law and named after the regelwerk, and the UI spec only orders,
// groups, names, fills or hides.
import { hoofdletter, leesbaar, zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { tegenBesluit } from '@regelrecht/frontend-shared/chronolex/voorinvulling';
import { artikelNaam } from './keten';
import type { Indieningsvorm, Systeemvulling, Veld, Veldtekst, Vormen } from './types';

/** A field as a screen shows it. */
export interface Schermveld {
  veld: Veld;
  /** The name on the screen: the UI spec's `label`, or the field's name. */
  label: string;
  /** The line under it: the UI spec's `help`, or nothing. */
  help: string;
  /** The heading of the group it stands under: the UI spec's `group`, or none. */
  groep: string;
}

/** `veld` as a screen shows it, with the texts the UI spec gives it. */
export function schermveld(veld: Veld, teksten: Vormen['teksten'] = {}): Schermveld {
  const tekst: Veldtekst = teksten[veld.naam] ?? {};
  return {
    // The article a field comes from, in words for the screen, such as `Awb artikel 4:2`.
    veld: veld.basis ? { ...veld, basis: artikelNaam(veld.basis) } : veld,
    label: tekst.label ?? hoofdletter(leesbaar(veld.naam)),
    help: tekst.help ?? '',
    groep: tekst.group ?? '',
  };
}

/** The field `naam` of a record as a screen shows it, where the law's own description is not at hand. */
export const kaalSchermveld = (naam: string, teksten: Vormen['teksten'] = {}): Schermveld =>
  schermveld({ naam, type: null }, teksten);

/** The fields the login fills: those the system fills without a value or a source. */
export const doorInloggen = (vorm: Indieningsvorm): Set<string> =>
  new Set(
    Object.entries(vorm.systeem ?? {})
      .filter(([, hoe]) => (hoe.soort ? hoe.soort === 'inloggen' : !('waarde' in hoe) && !hoe.bron))
      .map(([naam]) => naam),
  );

/** `<regelwerk>#<article>#<name>` without its last part: the article of a rule or a fact. */
export const artikelVan = (bron: string): string => bron.split('#').slice(0, 2).join('#');

/**
 * Where the value of a field the system fills comes from, as a line under
 * it: the login, the clock, a fixed value, the decision the law names, the
 * means of login, or the article of the rule whose outcome it is.
 */
export function herkomst(hoe: Systeemvulling): string {
  if (hoe.bron) return `Volgt uit ${artikelNaam(artikelVan(hoe.bron))}.`;
  switch (hoe.soort) {
    case 'datum':
      return 'De dag van indiening, volgens de klok van het systeem.';
    case 'gevraagd_besluit':
      return 'Het besluit dat de wet bij deze aanvraag noemt.';
    case 'verwezen':
      return 'Het besluit waarover deze aanvraag gaat.';
    case 'inlogmiddel':
      return 'Het middel waarmee de aanvrager inlogde.';
    case 'waarde':
      return 'Hetzelfde bij elke aanvraag.';
    default:
      return 'waarde' in hoe && hoe.soort !== 'inloggen' ? 'Hetzelfde bij elke aanvraag.' : 'Uit de inlog van de aanvrager.';
  }
}

/** A field of the application form: one the applicant fills, or one the system fills, shown as it is filled. */
export interface Formulierveld extends Schermveld {
  /** Set when the system fills the field: its value and where it comes from. */
  systeem?: { waarde: unknown; herkomst: string };
}

/** Fields under one heading, as the UI spec groups them (`group`); a field without one has no heading. */
export interface Veldgroep<V = Formulierveld> {
  titel: string;
  velden: V[];
}

/** The application form of `vorm`: its fields in groups, and those of them the applicant fills. */
export interface Aanvraagformulier {
  groepen: Veldgroep[];
  invullen: Schermveld[];
}

/**
 * `velden` in the order the UI spec names them (`volgorde`); a field it does
 * not name follows, in the order it came in. `naam` gives a field's name.
 */
export function opVolgorde<V extends { naam: string }>(velden: V[], volgorde?: string[]): V[];
export function opVolgorde<V>(velden: V[], volgorde: string[] | undefined, naam: (v: V) => string): V[];
export function opVolgorde<V>(
  velden: V[],
  volgorde: string[] = [],
  // The overloads above make a caller without `naam` pass fields that have one.
  naam: (v: V) => string = (v) => (v as { naam: string }).naam,
): V[] {
  const plek = (v: V) => {
    const i = volgorde.indexOf(naam(v));
    return i < 0 ? volgorde.length : i;
  };
  return velden
    .map((v, i) => ({ v, i }))
    .sort((a, b) => plek(a.v) - plek(b.v) || a.i - b.i)
    .map(({ v }) => v);
}

/**
 * `velden`, already in order, under the headings of their groups. A group
 * stands where its first field stands, and every later field of it joins it
 * there, so an order that interrupts a group never shows its heading twice.
 * A field without a group stays where it stands, under no heading.
 */
export function inGroepen<V extends { groep: string }>(velden: V[]): Veldgroep<V>[] {
  const groepen: Veldgroep<V>[] = [];
  for (const veld of velden) {
    const laatste = groepen[groepen.length - 1];
    const groep = veld.groep ? groepen.find((g) => g.titel === veld.groep) : laatste?.titel === '' ? laatste : undefined;
    if (groep) groep.velden.push(veld);
    else groepen.push({ titel: veld.groep, velden: [veld] });
  }
  return groepen;
}

/** `velden` of a form in the order of the UI spec, under the headings of its groups. */
export const geordend = <V extends Schermveld>(velden: V[], volgorde?: string[]): Veldgroep<V>[] =>
  inGroepen(opVolgorde(velden, volgorde, (s) => s.veld.naam));

/**
 * Per field of the application `vorm` that the system fills, where its value
 * comes from ([`herkomst`]): for a screen that shows a recorded application.
 */
export const herkomsten = (vorm: Indieningsvorm | undefined): Record<string, string> =>
  Object.fromEntries(Object.entries(vorm?.systeem ?? {}).map(([naam, hoe]) => [naam, herkomst(hoe)]));

/**
 * The application form of `vorm`. Every field of the law but those the UI
 * spec hides, in the order of the UI spec and under the headings of its
 * groups. The applicant fills a field the system does not; a field the
 * system fills shows read-only with the value the server will record (the
 * server gives it to a citizen with the forms), or for the login field the
 * identifier of the login (`identiteit`), and where that value comes from.
 * The form never sends such a field ([`aanvraagVelden`]).
 */
export function aanvraagformulier(
  vorm: Indieningsvorm,
  teksten: Vormen['teksten'] = {},
  identiteit: string | null = null,
  volgorde: string[] = [],
): Aanvraagformulier {
  const systeem = vorm.systeem ?? {};
  const verborgen = vorm.verborgen ?? [];
  const inloggen = doorInloggen(vorm);
  const velden: Formulierveld[] = opVolgorde(
    vorm.velden.filter((v) => !verborgen.includes(v.naam)),
    volgorde,
  ).map((v) => {
    const hoe = systeem[v.naam];
    const veld: Formulierveld = schermveld(v, teksten);
    if (!hoe) return veld;
    const waarde = 'waarde' in hoe ? hoe.waarde : inloggen.has(v.naam) && !hoe.inlog ? identiteit : undefined;
    return { ...veld, systeem: { waarde, herkomst: herkomst(hoe) } };
  });
  return { groepen: inGroepen(velden), invullen: velden.filter((v) => !v.systeem) };
}

/** The fields of a submission against the decision `besluit`, as the screen shows them (`tegenBesluit`). */
export const tegenBesluitVelden = (
  vorm: Indieningsvorm,
  besluit: { outputs?: Record<string, unknown>; fields?: Record<string, unknown> },
  teksten: Vormen['teksten'],
): Schermveld[] => tegenBesluit(vorm.velden, besluit).map((v) => schermveld(v, teksten));

/**
 * What a form starts with: a yes-or-no the law requires starts at no, as an
 * untouched switch shows it. Every other field starts empty, and one the law
 * lets be empty may stay so.
 */
export function beginwaarden(velden: Schermveld[]): Record<string, unknown> {
  return Object.fromEntries(
    velden.filter((s) => s.veld.type === 'boolean' && !s.veld.mag_leeg).map((s) => [s.veld.naam, false]),
  );
}

/**
 * The fields of an application as the server takes them: the form's values
 * without the empty ones and without any field the system fills, which the
 * server fills itself and refuses from the request.
 */
export function aanvraagVelden(vorm: Indieningsvorm, waarden: Record<string, unknown>): Record<string, unknown> {
  const uit: Record<string, unknown> = zonderLeeg(waarden);
  for (const naam of Object.keys(vorm.systeem ?? {})) delete uit[naam];
  return uit;
}
