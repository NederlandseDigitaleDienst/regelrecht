// What a form shows, from what the server derives from the law (`velden`)
// and what the UI spec of the case says of each field (`systeem`,
// `verborgen`, `teksten`). Nothing here names a field of a case: without a UI
// spec every field the law asks is on the form, named after the regelwerk,
// and the UI spec only adds to that or hides.
import { hoofdletter, leesbaar, zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { metSysteem, opHetFormulier, tegenBesluit } from '@regelrecht/frontend-shared/chronolex/voorinvulling';
import type { Indieningsvorm, Veld, Veldtekst, Vormen } from './types';

/** A field as a screen shows it. */
export interface Schermveld {
  veld: Veld;
  /** The name on the screen: the UI spec's `label`, or the field's name. */
  label: string;
  /** The line under it: the UI spec's `help`, or nothing. */
  help: string;
}

/** `veld` as a screen shows it, with the texts the UI spec gives it. */
export function schermveld(veld: Veld, teksten: Vormen['teksten'] = {}): Schermveld {
  const tekst: Veldtekst = teksten[veld.naam] ?? {};
  return {
    veld,
    label: tekst.label ?? hoofdletter(leesbaar(veld.naam)),
    help: tekst.help ?? '',
  };
}

/** The fields the login fills: those the system fills without a value or a source. */
export const doorInloggen = (vorm: Indieningsvorm): Set<string> =>
  new Set(
    Object.entries(vorm.systeem ?? {})
      .filter(([, hoe]) => !('waarde' in hoe) && !hoe.bron)
      .map(([naam]) => naam),
  );

/** A field of the application the system fills, shown as it is filled. */
export interface Systeemveld extends Schermveld {
  /** What fills it: the login's identifier, or the value the system knows. */
  waarde: unknown;
  /** Whether the login fills it. */
  uitInlog: boolean;
}

/** The application form of `vorm`: what the applicant fills, and what the system fills that the form shows. */
export interface Aanvraagformulier {
  invullen: Schermveld[];
  doorSysteem: Systeemveld[];
}

/**
 * The application form of `vorm`. The applicant fills every field of the law
 * but those the system fills and those the UI spec hides; a field the system
 * fills shows read-only, with the identifier of the login (`identiteit`) or
 * the value the system knows, unless the UI spec hides it.
 */
export function aanvraagformulier(
  vorm: Indieningsvorm,
  teksten: Vormen['teksten'],
  identiteit: string | null,
): Aanvraagformulier {
  const systeem = vorm.systeem ?? {};
  const verborgen = vorm.verborgen ?? [];
  const invullen = opHetFormulier(vorm.velden, verborgen, new Set(Object.keys(systeem))).map((v) =>
    schermveld(v, teksten),
  );
  const inloggen = doorInloggen(vorm);
  const doorSysteem = vorm.velden
    .filter((v) => v.naam in systeem && !verborgen.includes(v.naam))
    .map((v) => ({
      ...schermveld(v, teksten),
      waarde: inloggen.has(v.naam) ? identiteit : systeem[v.naam]?.waarde,
      uitInlog: inloggen.has(v.naam),
    }));
  return { invullen, doorSysteem };
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
 * with what the system fills over them, without the empty ones and without
 * the fields the login fills, which the server fills itself and refuses from
 * the request.
 */
export function aanvraagVelden(vorm: Indieningsvorm, waarden: Record<string, unknown>): Record<string, unknown> {
  const uit: Record<string, unknown> = zonderLeeg(metSysteem(waarden, vorm.systeem ?? {}, {}));
  for (const naam of doorInloggen(vorm)) delete uit[naam];
  return uit;
}
