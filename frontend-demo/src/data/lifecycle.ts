/**
 * De levensloop van een besluit, zoals de Awb die geeft (RFC-008).
 *
 * De Awb bepaalt dat een beschikking niet op één moment ontstaat maar door
 * fasen loopt: een aanvraag, de behandeling ervan, het besluit, de
 * bekendmaking, en daarna de bezwaartermijn. De fasen staan niet hier maar in
 * de wet (`procedure:` in de Awb-YAML); de engine loopt ze af en zegt bij elke
 * fase welk gegeven hij nog niet heeft.
 *
 * Dit bestand is de kant van de demo: hoe een zaak in het scherm zich verhoudt
 * tot die levensloop. RFC-008 zegt daarover twee dingen die hier alles bepalen.
 *
 * Ten eerste: "the besluit itself is the state container; there is no separate
 * 'case' or 'zaak' abstraction". De zaak in de demo ís het besluit. Er staat
 * dus geen tweede boekhouding naast de levensloop — de fase is de status, en
 * niet iets wat ernaast wordt bijgehouden en uit de pas kan gaan lopen.
 *
 * Ten tweede: "the engine itself remains stateless ... the orchestration layer
 * persists the besluit state and feeds new inputs when they become available".
 * De demo is die orkestratielaag. Zij bewaart `stageState` bij de zaak en levert
 * aan wat de engine vraagt, op het moment dat het er is.
 *
 * Wat een fase is en wat een uitkomst is, is niet hetzelfde. "Toegekend" en
 * "afgewezen" zijn geen fasen: het zijn twee uitkomsten van dezelfde fase
 * (BESLUIT). Daarom blijft `approved` een veld op het besluit en wordt alleen
 * het moment door de fase beschreven.
 */

import { decisionOf } from '@regelrecht/frontend-shared/produces';
import type { Produces } from '@regelrecht/frontend-shared/produces';

/**
 * A case (zaak), as far as this module reads it. `stageState` is what the
 * engine's procedure run returns, with the engine's own keys.
 */
export interface LifecycleCase {
  status?: string;
  withdrawnAt?: unknown;
  objection?: unknown;
  stageState?: {
    current_stage?: string | null;
    accumulated_outputs?: Record<string, unknown> | null;
  } | null;
}

/** The outcomes the Awb added to a decision (see `awbOutcomes`); `null` when not reached. */
export interface AwbOutcomes {
  motiveringVereist: unknown;
  bezwaartermijnWeken: unknown;
  bezwaartermijnStart: unknown;
  bezwaartermijnEinde: unknown;
}

/**
 * Of een burger of bedrijf deze regeling kan aanvragen. Een beschikking wel,
 * een aanslag niet: die legt de overheid op (Claudia's precario, de IB), ook al
 * is het een beschikking. Tegel en aanvraag vragen het hier, zodat niet de ene
 * knop het weet en de andere niet.
 *
 * @param produces `execution.produces`, in either form
 */
export function canBeApplied(produces: Produces): boolean {
  const decision = decisionOf(produces);
  return decision?.legal_character === 'BESCHIKKING' && decision?.decision_type !== 'AANSLAG';
}

/**
 * Of een aanvraag onder deze regeling altijd langs een behandelaar gaat.
 * Een terrasvergunning verleent de gemeente na beoordeling; de demo besluit
 * dan niet zelf, ook niet als de wet uitkomt op "voldoet". Welke regelingen dat
 * zijn staat in `review_laws` in demo-config.yaml, per dienst, zoals
 * `hidden_laws`.
 */
export function reviewedByCaseworker(
  lawEntry: { service: string; law_path: string },
  config: { review_laws?: Record<string, string[] | undefined> | null } | null | undefined,
): boolean {
  return (config?.review_laws?.[lawEntry.service] ?? []).some(
    (p) => lawEntry.law_path === p || lawEntry.law_path.startsWith(`${p}/`),
  );
}

/** De fasen van de beschikkingsprocedure, in de volgorde van de Awb. */
export const STAGES: readonly string[] = ['AANVRAAG', 'BEHANDELING', 'BESLUIT', 'BEKENDMAKING', 'BEZWAAR'];

/**
 * Welke fase eerder komt dan welke. Alleen voor "is deze zaak al voorbij X",
 * nooit om zelf een volgende fase te kiezen: dat doet de engine, uit de wet.
 */
export function stageIndex(stage: string | null | undefined): number {
  // `indexOf` of a missing stage is -1, as for any name not in the list.
  return STAGES.indexOf(stage as string);
}

export function reachedStage(caseRecord: LifecycleCase | null | undefined, stage: string): boolean {
  const at = stageIndex(caseRecord?.stageState?.current_stage);
  return at >= 0 && at >= stageIndex(stage);
}

/**
 * De status waarop de rest van de demo stuurt, afgeleid uit de fase.
 *
 * Afgeleid en niet apart bewaard: twee velden die hetzelfde zouden moeten
 * zeggen, gaan een keer uit de pas lopen. De fase is de bron.
 *
 * Een zaak zonder levensloop (een wet die geen beschikking geeft, of een zaak
 * uit opgeslagen staat van vóór deze verandering) houdt de status die er al
 * stond. Zo blijft een demo die midden in een verhaal staat gewoon werken.
 */
export function statusOf(caseRecord: LifecycleCase | null | undefined): string {
  if (caseRecord?.withdrawnAt) return 'WITHDRAWN';
  const stage = caseRecord?.stageState?.current_stage;
  if (!stage) return caseRecord?.status ?? 'IN_REVIEW';
  // `current_stage` is de fase waar de levensloop vóór staat, niet de fase die
  // hij heeft afgerond: de engine zet hem op de volgende fase en kijkt dán pas
  // of hij de gegevens daarvan heeft (`execute_stage_internal`). Een zaak die
  // op BESLUIT staat wacht dus nog op de besluitdatum — de haken van artikel
  // 3:46 en 6:7 hebben niet gevuurd, en er ís nog geen besluit.
  //
  // Dat verschil verkeerd lezen draait de demo om: elke aanvraag die naar een
  // behandelaar gaat, zou meteen als besloten op het portaal staan.
  if (stage === 'AANVRAAG') return 'SUBMITTED';
  if (stage === 'BEHANDELING' || stage === 'BESLUIT') return 'IN_REVIEW';
  // Voorbij BESLUIT is het besluit genomen. Of het toe- of afwijst zegt
  // `approved`, niet de fase.
  return 'DECIDED';
}

/**
 * Of de burger nu bezwaar kan maken.
 *
 * Niet "er is besloten", maar "de bezwaartermijn loopt". Awb 6:8 laat die
 * termijn beginnen op de dag ná de bekendmaking, dus vóór de bekendmaking is er
 * niets om bezwaar tegen te maken — het besluit is de belanghebbende dan nog
 * niet eens meegedeeld.
 */
export function objectionOpen(caseRecord: LifecycleCase | null | undefined): boolean {
  return reachedStage(caseRecord, 'BEZWAAR') && !caseRecord?.objection;
}

/**
 * De uitkomsten die de Awb aan dit besluit heeft toegevoegd, als de levensloop
 * ver genoeg is om ze te hebben.
 *
 * Ze komen uit de wet en niet uit dit bestand: `termijn_weken` van Awb
 * 6:7, de begin- en einddatum van Awb 6:8, de motiveringsplicht van Awb 3:46.
 * Staat er een afwijkende termijn in een bijzondere wet ("in afwijking van
 * artikel 6:7"), dan is dat hier al in verwerkt zonder dat iemand het hoeft te
 * weten.
 */
export function awbOutcomes(caseRecord: LifecycleCase | null | undefined): AwbOutcomes {
  const out = caseRecord?.stageState?.accumulated_outputs ?? {};
  return {
    motiveringVereist: out.motivering_vereist ?? null,
    bezwaartermijnWeken: out.termijn_weken ?? null,
    bezwaartermijnStart: out.termijn_startdatum ?? null,
    bezwaartermijnEinde: out.termijn_einddatum ?? null,
  };
}
