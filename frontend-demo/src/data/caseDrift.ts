/**
 * Of een lopende aanvraag nog klopt met wat de wet nu zegt.
 *
 * Dit is geen opgeslagen toestand en geen herberekening die ergens draait: het
 * is een vergelijking tussen wat de burger aanvroeg (`claimedResult`) en wat
 * dezelfde wet nú uitrekent, met alles wat er sindsdien is opgegeven. Precies
 * wat de POC bij elke render deed: de tegels daar vergeleken
 * `result.<uitkomst>` met `current_case.claimed_result.<uitkomst>`
 * (`web/templates/partials/tiles/law/**​/*.html`).
 *
 * Dat het zo werkt is de kern en niet een implementatiedetail. Een besluit is
 * een vaststelling op een moment; er hoort niets achter de rug van de burger om
 * herrekend te worden. Wat wél moet gebeuren is dat hij ziet dat het niet meer
 * klopt, en dat hij er iets aan kan doen.
 *
 * Het werkt over de hele breedte omdat correcties per BSN staan en niet per
 * wet: een inkomenswijziging die bij de huurtoeslag is opgegeven, verandert
 * vanzelf de uitkomst van elke andere wet die datzelfde inkomen gebruikt. Een
 * toegekende zorgtoeslag komt daardoor uit zichzelf naar voren, zonder dat hier
 * iets "zorgtoeslag" of "huurtoeslag" bij naam kent.
 */

import { fieldSpec, formatValue, isUnknown } from './format';
import type { DeclaredField, LawDocLike } from './format';

/** A case (zaak), as far as this module reads it. */
export interface DriftCase {
  status: string;
  claimedResult?: Record<string, unknown> | null;
}

/** An evaluation, as far as this module reads it. */
interface DriftEvaluation {
  ok?: boolean;
  outputs?: Record<string, unknown> | null;
}

/** One outcome whose value now differs from what was applied for. */
export interface DriftChange {
  name: string;
  claimed: unknown;
  current: unknown;
}

/** The outcomes of a case that no longer hold, with the case itself. */
export interface Drift<R extends DriftCase = DriftCase> {
  case: R;
  changed: DriftChange[];
}

/** A changed outcome with its declared spec and both values formatted. */
export interface DriftRow extends DriftChange {
  spec: DeclaredField | null;
  was: string;
  now: string;
}

/** De statussen waarvan een uitkomst vastligt die niet meer kan kloppen. */
const LIVE_STATUSES = ['DECIDED', 'IN_REVIEW', 'SUBMITTED'];

/**
 * Of twee waarden van dezelfde uitkomst hetzelfde zeggen.
 *
 * Overgenomen uit de POC (`CaseManager._results_match`): getallen met 1%
 * tolerantie, waarbij nul alleen gelijk is aan nul, de rest exact. Die
 * tolerantie is er omdat een herberekening op een andere dag een cent kan
 * schelen, en een cent geen besluit hoort te heropenen.
 */
export function valuesMatch(a: unknown, b: unknown): boolean {
  if (typeof a === 'number' && typeof b === 'number') {
    return b === 0 ? a === 0 : Math.abs(b - a) / Math.abs(b) <= 0.01;
  }
  return JSON.stringify(a) === JSON.stringify(b);
}

/** Of twee volledige uitkomsten van dezelfde wet hetzelfde zeggen. */
export function resultsMatch(claimed: Record<string, unknown> | null | undefined, current: Record<string, unknown> | null | undefined): boolean {
  const keys = new Set([...Object.keys(claimed ?? {}), ...Object.keys(current ?? {})]);
  return [...keys].every((k) => valuesMatch(claimed?.[k], current?.[k]));
}

/**
 * De uitkomsten van deze zaak die niet meer kloppen, of `null` als er niets
 * aan de hand is.
 *
 * Geeft per uitkomst het bedrag van toen en dat van nu, zodat de tekst die de
 * burger leest ("Je vroeg eerder € x aan") uit de wet zelf komt en niet uit een
 * lijst met een regel per regeling.
 */
export function driftOf<R extends DriftCase>(caseRecord: R | null | undefined, evaluation: DriftEvaluation | null | undefined): Drift<R> | null {
  if (!caseRecord || !LIVE_STATUSES.includes(caseRecord.status)) return null;
  if (!evaluation?.ok) return null;
  const claimed = caseRecord.claimedResult ?? {};
  const current = evaluation.outputs ?? {};
  const changed = [...new Set([...Object.keys(claimed), ...Object.keys(current)])]
    // Een uitkomst die de wet niet kón bepalen zegt niets over het besluit: er
    // staat een vraag open (RFC-036), er is niets veranderd aan wat iemand
    // krijgt. Dat als wijziging melden stuurt de burger achter een verschil aan
    // dat er niet is.
    .filter((name) => !isUnknown(claimed[name]) && !isUnknown(current[name]))
    .filter((name) => !valuesMatch(claimed[name], current[name]))
    .map((name) => ({ name, claimed: claimed[name], current: current[name] }))
    // Het bedrag eerst: daar ging het besluit over. De rest eronder.
    .sort((a, b) => Number(typeof b.claimed === 'number') - Number(typeof a.claimed === 'number'));
  return changed.length ? { case: caseRecord, changed } : null;
}

/**
 * De rijen die het verschil laten zien, met de opmaak die de wet aan die
 * uitkomst geeft (een bedrag als bedrag, een ja/nee als ja/nee).
 */
export function driftRows(drift: Pick<Drift, 'changed'> | null | undefined, lawDoc: LawDocLike | null | undefined): DriftRow[] {
  return (drift?.changed ?? []).map((d) => {
    const spec = fieldSpec(lawDoc, d.name);
    return { ...d, spec, was: formatValue(d.claimed, spec), now: formatValue(d.current, spec) };
  });
}

/**
 * Eén zin voor de burger, met de bedragen erin.
 *
 * Staat hier en niet in een component, zodat de tegel en de aanvraag hetzelfde
 * zeggen. De POC had deze zin acht keer met de hand overgeschreven, met per
 * regeling het veld erin gehardcodeerd; hier komt het bedrag uit de wet.
 */
export function driftSentence(drift: Pick<Drift, 'changed'> | null | undefined, lawDoc: LawDocLike | null | undefined): string {
  const row = driftRows(drift, lawDoc)[0];
  if (!row) return '';
  return `Je vroeg eerder ${row.was} aan. Sindsdien zijn er gegevens gewijzigd waarmee deze regeling nu uitkomt op ${row.now}. Je besluit blijft gelden totdat je je aanvraag wijzigt.`;
}
