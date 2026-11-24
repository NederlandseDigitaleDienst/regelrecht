// What a form shows, from what the server derives from the law (`velden`)
// and what the UI spec of the case says of each field (`systeem`,
// `verborgen`, `teksten`, `volgorde`). Nothing here names a field of a case:
// without a UI spec every field the law asks is on the form, in the order
// of the law and named after the regelwerk, and the UI spec only orders,
// groups, names, fills or hides.
import { hoofdletter, leesbaar, zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { tegenBesluit } from '@regelrecht/frontend-shared/chronolex/voorinvulling';
import { artikelNaam, omschrijving, waardeTekst } from './keten';
import type { Casus, Gram, Indieningsvorm, Systeemvulling, Veld, Veldtekst, Vormen } from './types';

/** A field as a screen shows it. */
export interface Schermveld {
  veld: Veld;
  /** The name on the screen: the UI spec's `label`, or the field's name. */
  label: string;
  /** The line under it: the UI spec's `help`, or nothing. */
  help: string;
  /** The heading of the group it stands under: the UI spec's `group`, or none. */
  groep: string;
  /** A yes and a no in words (`when_true`, `when_false`), where the UI spec gives them. */
  ja?: string;
  nee?: string;
}

/**
 * `veld` as a screen shows it, with the texts the UI spec gives it. A form
 * speaks to whoever fills it in; a screen that shows a recorded submission
 * (`stuk`) takes the UI spec's `record` texts, and the form's where it gives
 * none.
 */
export function schermveld(veld: Veld, teksten: Vormen['teksten'] = {}, stuk = false): Schermveld {
  const eigen: Veldtekst = teksten?.[veld.naam] ?? {};
  // The server leaves out what the UI spec does not give, so a key `record` lacks keeps the form's.
  const tekst: Veldtekst = stuk ? { ...eigen, ...eigen.record } : eigen;
  return {
    // The article a field comes from, in words for the screen, such as `Awb artikel 4:2`.
    veld: veld.basis ? { ...veld, basis: artikelNaam(veld.basis) } : veld,
    label: tekst.label ?? hoofdletter(leesbaar(veld.naam)),
    help: tekst.help ?? '',
    groep: tekst.group ?? '',
    ja: tekst.when_true,
    nee: tekst.when_false,
  };
}

/** The field `naam` of a record as a screen that shows it names it, where the law's own description is not at hand. */
export const kaalSchermveld = (naam: string, teksten: Vormen['teksten'] = {}): Schermveld =>
  schermveld({ naam, type: null }, teksten, true);

/**
 * The value `v` of the field `naam` of a record in words, as it stands on its
 * own in a row: a yes or no as the UI spec names it (under `record`, else on
 * the form), anything else
 * as `waardeTekst` does, with a capital.
 */
export function stukWaarde(naam: string, v: unknown, teksten: Vormen['teksten'] = {}, noem?: (id: string) => string | undefined): string {
  const t = teksten?.[naam];
  const ja = t?.record?.when_true ?? t?.when_true;
  const nee = t?.record?.when_false ?? t?.when_false;
  if (v === true && ja) return ja;
  if (v === false && nee) return nee;
  return hoofdletter(waardeTekst(v, noem));
}

/** The fields the login fills: those the system fills without a value or a source. */
export const doorInloggen = (vorm: Indieningsvorm): Set<string> =>
  new Set(
    Object.entries(vorm.systeem ?? {})
      .filter(([, hoe]) => (hoe.soort ? hoe.soort === 'inloggen' : !('waarde' in hoe) && !hoe.bron))
      .map(([naam]) => naam),
  );

/** The field the login fills with the applicant's name (`inlog: naam`), if the form has one. */
export const naamVeld = (vorm: Indieningsvorm | undefined): string | undefined =>
  Object.entries(vorm?.systeem ?? {}).find(([, hoe]) => hoe.soort === 'inloggen' && hoe.inlog === 'naam')?.[0];

/**
 * Whether a submission is an application: the case's own, which refers to no
 * other record, or a request after a grant (`na_toekenning.aanvragen`), such
 * as a lost card. An objection or an appeal refers to the decision it
 * contests.
 */
export function isAanvraag(g: Gram, casus: Casus): boolean {
  return (!g.refers_to && !g.received) || !!casus.na_toekenning?.aanvragen?.some((a) => a.artikel === g.basis);
}

/**
 * What a submission asks for, in words: the title the case gives a request
 * after a grant, such as a lost card; for the case's own application the
 * title of the case; for anything else, such as an objection, its kind before
 * that title.
 */
export function gevraagd(g: Gram, casus: Casus): string {
  const na = casus.na_toekenning?.aanvragen?.find((a) => a.artikel === g.basis);
  if (na) return na.titel;
  const soort = omschrijving(g, casus, 'indiening');
  if (!casus.titel) return soort;
  return isAanvraag(g, casus) ? casus.titel : `${soort} ${casus.titel.toLowerCase()}`;
}

/**
 * The applicant's name a submission carries, as its form has the login fill
 * it; else the name on the record it links to (`refers_to`, `after`,
 * `received`), however many links away: an objection is about the decision
 * on an application, and that application holds the name.
 */
export function aanvragerNaam(g: Gram, vormen: Vormen | null | undefined, grammen: Gram[]): string | undefined {
  const opId = new Map(grammen.map((x) => [x.id, x]));
  const gezien = new Set<string>();
  const wachtrij = [g];
  for (let x = wachtrij.shift(); x; x = wachtrij.shift()) {
    if (gezien.has(x.id)) continue;
    gezien.add(x.id);
    const veld = naamVeld(vormen?.indieningen[x.basis]);
    const naam = veld ? x.fields?.[veld] : undefined;
    if (typeof naam === 'string' && naam.trim()) return naam.trim();
    for (const id of [x.refers_to, x.after, x.received?.id]) {
      const volgende = id ? opId.get(id) : undefined;
      if (volgende) wachtrij.push(volgende);
    }
  }
  return undefined;
}

/**
 * What a submission is called on a list and on its own screen: what it asks
 * for and who asks it, `Gehandicaptenparkeerkaart · P. Voorbeeld`. What it
 * asks for alone when no name is known.
 */
export function indieningTitel(g: Gram, casus: Casus, vormen: Vormen | null | undefined, grammen: Gram[] = []): string {
  const wat = gevraagd(g, casus);
  const naam = aanvragerNaam(g, vormen, grammen);
  return naam ? `${wat} · ${naam}` : wat;
}

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
  /**
   * Set when the system fills the field: its value and where it comes from.
   * With `aanpasbaar` it is a prefill (`fill: {from}`): the applicant may
   * change it, and the form sends it as theirs.
   */
  systeem?: { waarde: unknown; herkomst: string; aanpasbaar?: boolean };
  /** Set on an input of a field the screen asks in parts: the law field it answers, and the part. */
  deel?: { veld: string; naam: string; autocomplete?: string; keyboard?: string; width?: string };
  /** The form does not submit without it (`required` in the UI spec); a part of such a field is too. */
  verplicht?: boolean;
}

/** The key a part of `veld` holds its value under, which is no field of the law. */
export const deelsleutel = (veld: string, deel: string): string => `${veld}.${deel}`;

/** `veld` as the screen asks it: itself, or one input per part the UI spec names. */
function metDelen(veld: Formulierveld, delen: Vormen['delen'] = {}): Formulierveld[] {
  const stukken = delen?.[veld.veld.naam];
  if (!stukken?.length) return [veld];
  return stukken.map((d) => ({
    ...veld,
    veld: { ...veld.veld, naam: deelsleutel(veld.veld.naam, d.naam) },
    label: d.label ?? d.naam,
    help: d.help ?? '',
    deel: { veld: veld.veld.naam, naam: d.naam, autocomplete: d.autocomplete, keyboard: d.keyboard, width: d.width },
  }));
}

/** Fields under one heading, as the UI spec groups them (`group`); a field without one has no heading. */
export interface Veldgroep<V = Formulierveld> {
  titel: string;
  velden: V[];
}

/** The application form of `vorm`: its fields in groups, and those of them the applicant fills. */
export interface Aanvraagformulier {
  groepen: Veldgroep[];
  /** The fields the applicant fills: a part of a field is one of them. */
  invullen: Formulierveld[];
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
 * [`herkomsten`] for a screen that shows one recorded submission: only a
 * value a rule derived, with the article of that rule, which a decider may
 * want to follow. The rest says how the form works (the login, the clock, a
 * value that is the same on every submission) and nothing about this one.
 */
export const herkomstenOpStuk = (vorm: Indieningsvorm | undefined): Record<string, string> =>
  Object.fromEntries(Object.entries(vorm?.systeem ?? {}).flatMap(([naam, hoe]) => (hoe.bron ? [[naam, herkomst(hoe)]] : [])));

/**
 * The application form of `vorm`. Every field of the law but those the UI
 * spec hides, in the order of the UI spec and under the headings of its
 * groups. The applicant fills a field the system does not; a field the
 * system fills shows read-only with the value the server will record (the
 * server gives it to a citizen with the forms), or for the login field the
 * identifier of the login (`identiteit`), and where that value comes from.
 * The form never sends such a field ([`aanvraagVelden`]). A field the UI
 * spec requires (`verplicht`) the applicant has to fill, every part of it.
 */
export function aanvraagformulier(
  vorm: Indieningsvorm,
  teksten: Vormen['teksten'] = {},
  identiteit: string | null = null,
  volgorde: string[] = [],
  delen: Vormen['delen'] = {},
  verplicht: Vormen['verplicht'] = [],
  bewerkt: ReadonlySet<string> = new Set(),
): Aanvraagformulier {
  const systeem = vorm.systeem ?? {};
  const voorgevuld = vorm.voorgevuld ?? {};
  const verborgen = vorm.verborgen ?? [];
  const inloggen = doorInloggen(vorm);
  const velden: Formulierveld[] = opVolgorde(
    vorm.velden.filter((v) => !verborgen.includes(v.naam)),
    volgorde,
  ).map((v) => {
    const hoe = systeem[v.naam];
    const veld: Formulierveld = schermveld(v, teksten);
    const voor = voorgevuld[v.naam];
    // A prefill with a value shows as known, read-only with its source, until
    // the applicant chooses to change it (UI pattern edit-in-a-sheet).
    if (!hoe && voor && voor.waarde != null && !bewerkt.has(v.naam)) {
      const bron = voor.bron ? ` uit ${artikelNaam(artikelVan(voor.bron))}` : '';
      return { ...veld, systeem: { waarde: voor.waarde, herkomst: `Vooringevuld${bron}; u kunt het aanpassen.`, aanpasbaar: true } };
    }
    if (!hoe) return verplicht?.includes(v.naam) ? { ...veld, verplicht: true } : veld;
    const waarde = 'waarde' in hoe ? hoe.waarde : inloggen.has(v.naam) && !hoe.inlog ? identiteit : undefined;
    return { ...veld, systeem: { waarde, herkomst: herkomst(hoe) } };
  })
    .flatMap((v) => (v.systeem ? [v] : metDelen(v, delen)));
  return { groepen: inGroepen(velden), invullen: velden.filter((v) => !v.systeem) };
}

/** What the form starts with besides its empty fields: the value of every prefill (`voorgevuld`). */
export const voorgevuldeWaarden = (vorm: Indieningsvorm | undefined): Record<string, unknown> =>
  Object.fromEntries(
    Object.entries(vorm?.voorgevuld ?? {})
      .filter(([, v]) => v.waarde != null)
      .map(([naam, v]) => [naam, v.waarde]),
  );

/** The fields of a submission against the decision `besluit`, as the screen shows them (`tegenBesluit`). */
export const tegenBesluitVelden = (
  vorm: Indieningsvorm,
  besluit: { outputs?: Record<string, unknown>; fields?: Record<string, unknown> },
  teksten: Vormen['teksten'],
): Schermveld[] => tegenBesluit(vorm.velden, besluit).map((v) => schermveld(v, teksten));

/**
 * What a form starts with: a yes-or-no the law requires starts at no, as an
 * untouched checkbox shows it. Every other field starts empty, and one the law
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
export function aanvraagVelden(
  vorm: Indieningsvorm,
  waarden: Record<string, unknown>,
  delen: Vormen['delen'] = {},
): Record<string, unknown> {
  const uit: Record<string, unknown> = zonderLeeg(waarden);
  // A field the screen asks in parts holds its value per part; the law asks
  // one, so the parts join in the order the UI spec names them.
  for (const [veld, stukken] of Object.entries(delen ?? {})) {
    const samen = stukken
      .map((d) => uit[deelsleutel(veld, d.naam)])
      .filter((w) => w !== undefined && w !== null && String(w).trim() !== '')
      .map((w) => String(w).trim())
      .join(', ');
    for (const d of stukken) delete uit[deelsleutel(veld, d.naam)];
    if (samen) uit[veld] = samen;
  }
  for (const naam of Object.keys(vorm.systeem ?? {})) delete uit[naam];
  return uit;
}
