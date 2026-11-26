/**
 * The application prefilled from the sources, and filled by the system, as
 * the UI spec of a source says (packages/poc-chronolexografie/src/ui_spec.rs):
 * per field which source prefills it (`prefill`, `<regelwerk>#<article>#<output>`),
 * which the system fills (`systeem`) and which the form hides (`verborgen`).
 * Shared by the poc and RegelHet.
 *
 * A prefilled value is a proposal: the applicant can change it, and what they
 * submit is what the application holds. The field says where the value comes
 * from, or that the applicant changed it.
 *
 * A field the system fills is no proposal: the applicant does not type it.
 * Per field it says `{ waarde }` for a value the system knows itself,
 * `{ bron }` for a source the cell computes as it prefills, and neither for
 * the login (`{ soort: 'inloggen' }`). `soort` names the kind of fill
 * (`kern::door_het_systeem`); these functions read only `waarde` and `bron`.
 */
import { isUnknown } from '../values';
import { alsInvoer, bronRij, verschilTekst, zelfdeTabel } from './tabel';
import type { Kolom, Rij } from './tabel';
import { leesbaar, toon, waarde as leesbareWaarde } from './waarden';

/** What a source gave for a field (`kern::voorinvullen`). */
export interface Voorgevuld {
  bron?: string;
  waarde?: unknown;
  herkomst?: { door: string; antwoord?: unknown }[];
  fout?: string;
}

/**
 * Where the system takes the value of a field from (`kern::door_het_systeem`):
 * the login, the clock, a fixed value, the decision the law names, the means
 * of login, or a source (`bron`).
 */
export type Systeemsoort = 'inloggen' | 'datum' | 'waarde' | 'gevraagd_besluit' | 'inlogmiddel' | 'verwezen' | 'bron';

/** How the system fills a field. */
export interface Systeemvulling {
  /** The kind of fill. */
  soort?: Systeemsoort;
  /**
   * A value the system knows itself; from a system that computes them for
   * the one who submits, such as the RegelHet server, any kind's value.
   */
  waarde?: unknown;
  /** The source whose outcome fills it, `<regelwerk>#<article>#<output>`. */
  bron?: string;
  /** For a source, what it takes of the submission, per parameter (`date`, `login_method`, ...). */
  parameters?: Record<string, string>;
  /** The claim of the login that fills it, when not the field's own name. */
  inlog?: string;
}

/** A field of a form, as far as these functions read it. */
export interface FormVeld {
  naam: string;
}

type Waarden = Record<string, unknown>;

/**
 * What the application form starts from. `scenario` is what a scenario gives
 * the form (the case's example with the step's own values over it), `invoer`
 * the step's own values alone, and `voorgevuld` what the sources gave. A
 * prefilled field takes the source over the example, since the example is no
 * choice of the applicant; a value the step sets is, and wins.
 */
export function beginWaarden(
  scenario: Waarden | null | undefined,
  invoer: Waarden | null | undefined,
  voorgevuld: Record<string, Voorgevuld> | null | undefined,
): Waarden {
  const eigen = Object.fromEntries(
    Object.keys(invoer ?? {})
      .filter((naam) => naam in (scenario ?? {}))
      .map((naam) => [naam, scenario?.[naam]]),
  );
  return { ...(scenario ?? {}), ...voorgevuldeWaarden(voorgevuld), ...eigen };
}

/**
 * What the sources get from the form `waarden`: every field with a value,
 * prefilled ones too. A source may read another prefilled field, as the
 * seats read the designation; a value the applicant changes then reaches it.
 * Asking again with what a source gave changes nothing, so the form settles.
 */
export function bronInvoer(waarden: Waarden | null | undefined): Waarden {
  return Object.fromEntries(Object.entries(waarden ?? {}).filter(([, w]) => w !== undefined));
}

/** Whether a source gave the field a value: not empty, and not unknown (RFC-036). */
const gegeven = (v: Voorgevuld | undefined): v is Voorgevuld =>
  v?.waarde !== null && v?.waarde !== undefined && !isUnknown(v.waarde);

/**
 * Per prefilled field the value a source gave, without the empty and the
 * unknown ones: an unknown value is no proposal, so the field stays as the
 * applicant left it. A table holds its decimal cells as numbers (tabel.ts
 * `alsInvoer`).
 */
export function voorgevuldeWaarden(voorgevuld: Record<string, Voorgevuld> | null | undefined): Waarden {
  return Object.fromEntries(
    Object.entries(voorgevuld ?? {})
      .filter(([, v]) => gegeven(v))
      .map(([naam, v]) => [naam, alsInvoer(v.waarde)]),
  );
}

/**
 * The line under a prefilled field: which cell the value comes from, or that
 * the applicant changed or emptied it and what the source gives, or why the
 * source gave nothing (`fout`, or an unknown value and what it misses).
 * Empty when no cell gave a value.
 * `celNaam` names a cell by its id. A table (a field with `kolommen`) counts
 * as the source's when it holds the same rows, in any order (tabel.ts); a
 * changed one says per row what the source gives.
 */
export function herkomstTekst(
  voorgevuld: Voorgevuld | null | undefined,
  waarde: unknown,
  celNaam: (cel: string) => string,
  kolommen?: Kolom[],
): string {
  if (voorgevuld?.fout) return `Niet vooringevuld: ${voorgevuld.fout}`;
  if (!voorgevuld || voorgevuld.waarde === null || voorgevuld.waarde === undefined) return '';
  const cellen = [...new Set((voorgevuld.herkomst ?? []).map((h) => celNaam(h.door)))];
  const wie = cellen.join(' en ') || 'de bron';
  if (isUnknown(voorgevuld.waarde)) return `Niet vooringevuld: ${wie} geeft ${leesbareWaarde(voorgevuld.waarde)}.`;
  const bronTabel = Array.isArray(kolommen) && Array.isArray(voorgevuld.waarde) ? (voorgevuld.waarde as Rij[]) : null;
  if (bronTabel && kolommen ? zelfdeTabel(bronTabel, waarde, kolommen) : waarde === voorgevuld.waarde) {
    const antwoorden = [...new Set((voorgevuld.herkomst ?? []).map((h) => leesbaar(h.antwoord)))];
    return `Ingevuld uit ${wie}${antwoorden.length ? ` (${antwoorden.join(', ')})` : ''}.`;
  }
  const hoofd = wie.charAt(0).toUpperCase() + wie.slice(1);
  if (waarde === undefined) {
    const bron =
      bronTabel && kolommen ? bronTabel.map((rij) => bronRij(rij, kolommen)).join('; ') : toon(voorgevuld.waarde);
    return `Door u leeggemaakt. ${hoofd} geeft: ${bron}.`;
  }
  if (bronTabel && kolommen) {
    return `Door u gewijzigd. ${hoofd} geeft ${verschilTekst(bronTabel, waarde as Rij[], kolommen)}.`;
  }
  return `Door u gewijzigd. ${hoofd} geeft: ${toon(voorgevuld.waarde)}.`;
}

/**
 * What the system fills (`systeem`), given the login: everything, unless the
 * case logs in through a login step and nobody is logged in (`sessie`). Such
 * an application does not come through the form, as one on paper, and holds
 * what the applicant gives.
 */
export const systeemVult = (
  inloggen: { inlogstap?: unknown } | null | undefined,
  sessie: unknown,
  systeem: Record<string, Systeemvulling> | null | undefined,
): Record<string, Systeemvulling> => (inloggen?.inlogstap && !sessie ? {} : (systeem ?? {}));

/** Per field the system fills from a source, that source: asked as a prefill is. */
export const systeemBronnen = (systeem: Record<string, Systeemvulling> | null | undefined): Record<string, string> =>
  Object.fromEntries(
    Object.entries(systeem ?? {}).flatMap(([naam, hoe]) => (hoe.bron ? [[naam, hoe.bron]] : [])),
  );

/**
 * What the application holds: the form's `waarden`, with what the system
 * fills over them, per field the value it knows, or what the source gave
 * (`voorgevuld`), and empty when the source gave nothing. That goes over
 * what a scenario or the example gives. A field the login fills keeps what
 * the login gave it (`doorInloggen`), and is empty when the login gives it
 * nothing. A field the form hides keeps its value: it is submitted, only not
 * shown.
 */
export function metSysteem(
  waarden: Waarden,
  systeem: Record<string, Systeemvulling> | null | undefined,
  voorgevuld: Record<string, Voorgevuld> | null | undefined,
  doorInloggen: Set<string> = new Set(),
): Waarden {
  const gegeven = voorgevuldeWaarden(voorgevuld);
  const uit = { ...waarden };
  for (const [naam, hoe] of Object.entries(systeem ?? {})) {
    if ('waarde' in hoe) uit[naam] = hoe.waarde ?? undefined;
    else if (hoe.bron) uit[naam] = gegeven[naam];
    else if (!doorInloggen.has(naam)) uit[naam] = undefined;
  }
  return uit;
}

/**
 * The fields the form shows of `velden`: not those the login fills
 * (`doorInloggen`), nor those the UI spec hides (`verborgen`).
 */
export const opHetFormulier = <V extends FormVeld>(
  velden: V[] | null | undefined,
  verborgen: string[] | null | undefined,
  doorInloggen: Set<string>,
): V[] => (velden ?? []).filter((v) => !doorInloggen.has(v.naam) && !(verborgen ?? []).includes(v.naam));

/**
 * The line under a field the system fills: the source its value comes from.
 * A value the system knows itself, or the login gives, gets no line.
 */
export const systeemTekst = (
  hoe: Systeemvulling | null | undefined,
  voorgevuld: Voorgevuld | null | undefined,
  celNaam: (cel: string) => string,
): string => (hoe?.bron ? herkomstTekst(voorgevuld, voorgevuld?.waarde, celNaam) : '');

/**
 * The fields of a submission against the decision `besluit`: those of its
 * form but what the decision already holds, such as the contested decision
 * and the term it set, which the cell takes from the decision itself.
 */
export const tegenBesluit = <V extends FormVeld>(
  velden: V[] | null | undefined,
  besluit: { outputs?: Waarden | null; fields?: Waarden | null },
): V[] =>
  (velden ?? []).filter((v) => !(v.naam in (besluit.outputs ?? {})) && !(v.naam in (besluit.fields ?? {})));
