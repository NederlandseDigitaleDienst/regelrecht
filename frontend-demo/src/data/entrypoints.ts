/**
 * Welke regelingen een portaal aan iemand voorlegt (RFC-038).
 *
 * Een entrypoint is een artikel dat een beschikking oplevert. De Awb trekt die
 * grens: artikel 1:3 noemt een besluit een schriftelijke beslissing van een
 * bestuursorgaan, een beschikking een besluit dat niet van algemene strekking
 * is, en een aanvraag een verzoek van een belanghebbende om een besluit te
 * nemen. Artikel 2 Wet op de zorgtoeslag geeft de verzekerde een aanspraak die
 * je bij een bestuursorgaan inroept; de penitentiaire beginselenwet stelt vast
 * dát iemand gedetineerd is. Het eerste hoort op een portaal, het tweede niet.
 *
 * Dat staat in de wet zelf, in `produces.legal_character`. Er is dus geen
 * apart veld voor nodig, en het oude `discoverable` is verdwenen: dat
 * beantwoordde drie vragen tegelijk en sprak de wetbestanden ernaast tegen.
 *
 * Over wie een regeling gáát, volgt uit de sleutel waarop ze rekent: een wet
 * over een onderneming vraagt om een KvK-nummer, een wet over een persoon om
 * een BSN. Ook dat staat al in de wet en hoeft niet apart verklaard te worden.
 */

import { legalCharacterOf, producedItems } from '@regelrecht/frontend-shared/produces';
import type { ExecutionLike, LawDocLike } from './format';

/** Who a regeling is about, read from the key it computes on (see `subjectOf`). */
export type Subject = 'BUSINESS' | 'CITIZEN';

/**
 * De uitkomsten die een burger aangaan.
 *
 * `BESCHIKKING`: er komt een besluit uit dat je aanvraagt en waartegen bezwaar
 * openstaat (de zorgtoeslag).
 *
 * `RECHTSPOSITIE`: een rechtspositie die van rechtswege ontstaat, zonder dat
 * een bestuursorgaan beslist en zonder dat iemand iets aanvraagt. Artikel B1
 * Kieswet zegt wie kiesgerechtigd is; dat is geen besluit, maar het gaat de
 * burger wel aan.
 *
 * Alles daarbuiten (`TOETS`, `WAARDEBEPALING`, `INFORMATIEF`) is een tussenstap
 * waar een andere wet op rekent.
 */
const PORTAL_CHARACTERS = new Set<string | null>(['BESCHIKKING', 'RECHTSPOSITIE']);

/** De uitvoeringsblokken van alle artikelen van een wet. */
function executions(lawDoc: LawDocLike | null | undefined): ExecutionLike[] {
  return (lawDoc?.articles ?? []).map((a) => a.machine_readable?.execution).filter((ex): ex is ExecutionLike => Boolean(ex));
}

/**
 * Levert deze wet ergens iets op dat de burger zelf aangaat?
 *
 * Eén artikel is genoeg: een wet hoort op het portaal omdat één van haar
 * artikelen daar hoort. De Wet op de zorgtoeslag heeft naast het recht
 * (artikel 2, een beschikking) ook een vermogenstoets (artikel 3) die anderen
 * consumeren.
 */
export function producesBeschikking(lawDoc: LawDocLike | null | undefined): boolean {
  return executions(lawDoc).some((ex) => producedItems(ex.produces).some((p) => PORTAL_CHARACTERS.has(legalCharacterOf(p))));
}

/**
 * Waar deze wet over gaat: `BUSINESS` als ze om een KvK-nummer vraagt,
 * `CITIZEN` als ze om een BSN vraagt, anders `null`.
 *
 * Vraagt een wet om allebei, dan wint het KvK-nummer: ze gaat dan over de
 * onderneming en de BSN is alleen wie er namens haar handelt.
 */
const BUSINESS_KEYS = ['kvk_nummer', 'loonheffingennummer'];

export function subjectOf(lawDoc: LawDocLike | null | undefined): Subject | null {
  const names = new Set<string>();
  for (const ex of executions(lawDoc)) {
    for (const p of ex.parameters ?? []) names.add(p.name);
  }
  if (BUSINESS_KEYS.some((k) => names.has(k))) return 'BUSINESS';
  if (names.has('bsn')) return 'CITIZEN';
  return null;
}

/**
 * De uitvoer waaraan een machtigingswet te herkennen is.
 *
 * Gezag, curatele, bewind en volmacht zijn rechtsposities die de demo gebruikt
 * om te bepalen wie namens wie mag handelen. Ze horen niet als regeling op het
 * portaal: je vraagt ze niet aan en er valt niets te ontvangen.
 *
 * Herkend aan wat ze leveren en niet aan een label op de wet: elke
 * machtigingswet vervult hetzelfde uitvoercontract (`delegation.ts`), en dat
 * contract is wat haar tot machtigingswet maakt.
 */
const DELEGATION_OUTPUTS = ['heeft_delegaties', 'subject_ids', 'delegation_types'];

export function isDelegationProvider(lawDoc: LawDocLike | null | undefined): boolean {
  const names = new Set<string>();
  for (const ex of executions(lawDoc)) {
    for (const o of ex.output ?? []) names.add(o.name);
  }
  return DELEGATION_OUTPUTS.every((n) => names.has(n));
}

/**
 * Hoort deze wet op het portaal van dit soort onderwerp?
 *
 * Wat een deployment vervolgens nog wegfiltert (`hidden_laws`) is een keuze
 * van die deployment en staat los van wat de wet zegt.
 */
export function isEntrypointFor(lawDoc: LawDocLike | null | undefined, wantedSubject: Subject | null): boolean {
  if (isDelegationProvider(lawDoc)) return false;
  return producesBeschikking(lawDoc) && subjectOf(lawDoc) === wantedSubject;
}
