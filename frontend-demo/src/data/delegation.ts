/**
 * Wie mag namens wie handelen.
 *
 * Machtiging staat niet in de app maar in de wet: elke wet die het
 * machtigingscontract vervult levert de delegaties die zij regelt.
 * Het gezag over een minderjarige komt uit het Burgerlijk Wetboek, het
 * vertegenwoordigen van een onderneming uit de Machtigingenwet, bewind en
 * curatele uit hun eigen wetten. De demo evalueert die wetten met dezelfde
 * engine als de rest van het portaal en zet de uitkomst om in een lijst.
 *
 * Elke provider-wet levert dezelfde uitvoer, als parallelle lijsten (de
 * standaardinterface uit de POC, `machine/delegation/manager.py`):
 *
 *   heeft_delegaties   boolean
 *   subject_ids        ['999200001', ...]
 *   subject_names      ['Kind 1 van der Meer', ...]
 *   subject_types      ['CITIZEN' | 'BUSINESS' | 'SELF', ...]
 *   delegation_types   ['OUDERLIJK_GEZAG', 'EIGENAAR', ...]
 *   permissions        [['LEZEN', 'CLAIMS_INDIENEN'], ...]
 *   valid_from_dates   ['2020-01-01', ...]
 *   valid_until_dates  ['2038-01-01' | null, ...]
 *
 * Een wet die niets oplevert zegt `heeft_delegaties: false`; een wet die de
 * engine niet kan uitvoeren telt niet mee en blokkeert de rest niet.
 */
import { t } from '../i18n/index.js';

import { isDelegationProvider, subjectOf } from './entrypoints';
import type { Subject } from './entrypoints';
import type { LawDocLike } from './format';
import type { DemoProfileConfig } from './loadCorpus';

/** A law that may provide delegations: the parts of a `LawEntry` this module reads. */
export interface ProviderLaw {
  id: string;
  name: string;
  service: string;
  doc?: LawDocLike | null;
}

/** The engine call this module makes, structurally. */
export interface DelegationEngine {
  executeMultipleWithTrace(
    lawId: string,
    outputs: string[],
    params: Record<string, unknown>,
    referenceDate: string,
  ): { outputs?: Record<string, unknown> | null } | null | undefined;
}

/** One person or business someone may act for, and with which rights. */
export interface Delegation {
  subjectId: string;
  subjectName: string;
  /** `CITIZEN`, `BUSINESS` or `SELF` */
  subjectType: string;
  delegationType: string | null;
  /** In `PERMISSION_ORDER`, without duplicates. */
  permissions: string[];
  validFrom: string | null;
  validUntil: string | null;
  /** The law this delegation comes from; null when merged from several. */
  lawId: string | null;
  lawName: string | null;
  service: string | null;
  /** Every law a merged `SELF` delegation came from. */
  sourceLaws?: { id: string | null; name: string | null }[];
}

/** A delegation as one law returns it, before its permissions are ordered. */
type RawDelegation = Omit<Delegation, 'permissions'> & { permissions: unknown[] };

/** A law the engine could not run for this person. */
export interface DelegationError {
  lawId: string;
  message: string;
}

/** De uitvoernamen van de standaardinterface, in de volgorde van de lijsten. */
const OUTPUTS: string[] = [
  'heeft_delegaties',
  'subject_ids',
  'subject_names',
  'subject_types',
  'delegation_types',
  'permissions',
  'valid_from_dates',
  'valid_until_dates',
];

/**
 * De rechten, van licht naar zwaar. Bepaalt de volgorde waarin ze getoond
 * worden en welk recht overblijft als twee wetten hetzelfde onderwerp
 * verschillend regelen.
 */
export const PERMISSION_ORDER: string[] = ['LEZEN', 'CLAIMS_INDIENEN', 'BESLUITEN_ONTVANGEN'];

/**
 * Wat een recht betekent, als sleutel in de woordenboeken.
 *
 * Een sleutel en geen zin: deze labels staan in een menu dat tijdens een demo
 * van taal wisselt, en de enum-waarden ernaast (`LEZEN`, `OUDERLIJK_GEZAG`)
 * komen uit de wet en blijven zoals ze zijn.
 */
export const PERMISSION_LABELS: Record<string, string> = {
  LEZEN: 'delegation.permission.read',
  CLAIMS_INDIENEN: 'delegation.permission.correct_and_apply',
  BESLUITEN_ONTVANGEN: 'delegation.permission.receive_decisions',
};

/** Wat een soort machtiging heet, als sleutel. */
export const DELEGATION_TYPE_LABELS: Record<string, string> = {
  EIGEN_ZAKEN: 'delegation.type.own_affairs',
  OUDERLIJK_GEZAG: 'delegation.type.parental_authority',
  GEZAMENLIJK_GEZAG: 'delegation.type.joint_authority',
  VOOGDIJ: 'delegation.type.guardianship',
  VOOGDIJ_INSTELLING: 'delegation.type.guardianship_institution',
  EIGENAAR: 'delegation.type.owner',
  VENNOOT: 'delegation.type.partner',
  BESTUURDER: 'delegation.type.director',
  GEVOLMACHTIGDE: 'delegation.type.authorised_agent',
  CURATOR: 'delegation.type.curator',
  BEWINDVOERDER: 'delegation.type.administrator',
  MENTOR: 'delegation.type.mentor',
  EXECUTEUR: 'delegation.type.executor',
};

/** Het label van een soort machtiging, in de taal die aan staat. */
export function delegationTypeLabel(type: string | null | undefined): string {
  // `String(type)` is the key JavaScript indexes with anyway.
  const key = DELEGATION_TYPE_LABELS[String(type)];
  return key ? t(key) : String(type ?? '');
}

/** Het label van een recht, in de taal die aan staat. */
export function permissionLabel(permission: string | null | undefined): string {
  const key = PERMISSION_LABELS[String(permission)];
  return key ? t(key) : String(permission ?? '');
}

/** De wetten die machtigingen leveren, in een vaste volgorde. */
export function delegationProviders<L extends ProviderLaw>(
  corpus: { latestById: Map<string, L> } | null | undefined,
): L[] {
  if (!corpus) return [];
  return [...corpus.latestById.values()]
    .filter((law) => isDelegationProvider(law.doc))
    .sort((a, b) => a.id.localeCompare(b.id));
}

/** De uitvoernamen die deze wet werkelijk declareert. */
function declaredOutputs(law: ProviderLaw): string[] {
  const names = new Set<string>(
    (law.doc?.articles ?? [])
      .flatMap((a) => a.machine_readable?.execution?.output ?? [])
      .map((o) => o.name),
  );
  return OUTPUTS.filter((name) => names.has(name));
}

/** Een lijstuitvoer als array, ook als de engine één waarde teruggaf. */
function asList(value: unknown): unknown[] {
  if (value === null || value === undefined) return [];
  return Array.isArray(value) ? value : [value];
}

/**
 * De delegaties die één wet voor deze persoon oplevert.
 */
function delegationsFromLaw(law: ProviderLaw, outputs: Record<string, unknown>): RawDelegation[] {
  if (!outputs.heeft_delegaties) return [];
  const ids = asList(outputs.subject_ids);
  const names = asList(outputs.subject_names);
  const types = asList(outputs.subject_types);
  const kinds = asList(outputs.delegation_types);
  const permissions = asList(outputs.permissions);
  const from = asList(outputs.valid_from_dates);
  const until = asList(outputs.valid_until_dates);

  // The engine returns these lists untyped; the provider interface declares
  // them as strings (dates as ISO text or null), and they are taken as such.
  return ids.map((id, i) => ({
    subjectId: String(id),
    subjectName: (names[i] as string | undefined) ?? String(id),
    // Zonder opgave is het een burger: dat is wat de meeste wetten regelen.
    subjectType: (types[i] as string | undefined) ?? 'CITIZEN',
    delegationType: (kinds[i] as string | null | undefined) ?? null,
    permissions: asList(permissions[i]),
    validFrom: (from[i] as string | null | undefined) ?? null,
    validUntil: (until[i] as string | null | undefined) ?? null,
    // Waar deze machtiging vandaan komt: de wet is de verantwoording.
    lawId: law.id,
    lawName: law.name,
    service: law.service,
  }));
}

/**
 * Een machtiging geldt alleen binnen haar termijn. Een wet die zelf al op de
 * peildatum rekent levert niets buiten de termijn, maar niet elke wet doet
 * dat, en een demo die een verlopen machtiging toont is misleidend.
 *
 * Dit werkt door in wat er te zien is, en dat is de bedoeling: Claudia's
 * café staat sinds 2025-01-15 in het handelsregister, dus op een
 * peildatum daarvóór heeft zij die machtiging nog niet en verdwijnt de keuze
 * uit de werkbalk. Wie zich afvraagt waarom de knop weg is bij een vroege
 * peildatum: dat is de wet, niet een fout.
 */
function isValidOn(delegation: Pick<RawDelegation, 'validFrom' | 'validUntil'>, referenceDate: string | null | undefined): boolean {
  if (!referenceDate) return true;
  if (delegation.validFrom && String(delegation.validFrom) > referenceDate) return false;
  if (delegation.validUntil && String(delegation.validUntil) < referenceDate) return false;
  return true;
}

/** De rechten in vaste volgorde, zonder dubbelen en zonder onbekende. */
function orderPermissions(permissions: unknown[]): string[] {
  const held = new Set(permissions.map(String));
  return PERMISSION_ORDER.filter((p) => held.has(p));
}

/**
 * Alle machtigingen van één persoon, over alle provider-wetten heen.
 *
 * `SELF` (handelen namens jezelf) wordt door meerdere wetten geleverd:
 * minderjarigheid en handelingsonbekwaamheid zeggen er allebei iets over. Ze
 * worden samengevoegd tot één, met de doorsnede van de rechten: de wet die het
 * meest beperkt wint, want een meerderjarige die onder curatele staat mag niet
 * meer omdat een andere wet zwijgt. Blijft er niets over, dan resteert LEZEN.
 *
 * @param engine        de WASM-engine
 * @param corpus        het geladen corpus
 * @param bsn           degene die handelt
 * @param referenceDate peildatum
 */
export function delegationsFor(
  engine: DelegationEngine | null | undefined,
  corpus: { latestById: Map<string, ProviderLaw> } | null | undefined,
  bsn: string | null | undefined,
  referenceDate: string,
): { delegations: Delegation[]; errors: DelegationError[] } {
  const delegations: RawDelegation[] = [];
  const errors: DelegationError[] = [];
  if (!engine || !corpus || !bsn) return { delegations: [], errors };

  for (const law of delegationProviders(corpus)) {
    const want = declaredOutputs(law);
    // Zonder `heeft_delegaties` spreekt de wet de interface niet; overslaan.
    if (!want.includes('heeft_delegaties')) continue;
    try {
      const result = engine.executeMultipleWithTrace(law.id, want, { bsn }, referenceDate);
      delegations.push(...delegationsFromLaw(law, result?.outputs ?? {}));
    } catch (err) {
      // The engine throws a string, an `{ error }` object or an Error.
      const e = err as { error?: unknown; message?: unknown } | string | null | undefined;
      const message = typeof e === 'string' ? e : e?.error ?? e?.message ?? JSON.stringify(e);
      errors.push({ lawId: law.id, message: String(message) });
    }
  }

  const valid = delegations.filter((d) => isValidOn(d, referenceDate));
  const selves = valid.filter((d) => d.subjectType === 'SELF');
  const others = valid.filter((d) => d.subjectType !== 'SELF');

  const merged: Delegation[] = [];
  const first = selves[0];
  if (first) {
    let permissions = new Set(orderPermissions(first.permissions));
    for (const d of selves.slice(1)) {
      const own = new Set(orderPermissions(d.permissions));
      permissions = new Set([...permissions].filter((p) => own.has(p)));
    }
    merged.push({
      ...first,
      subjectId: bsn,
      subjectName: 'Mezelf',
      subjectType: 'SELF',
      delegationType: 'EIGEN_ZAKEN',
      permissions: permissions.size ? PERMISSION_ORDER.filter((p) => permissions.has(p)) : ['LEZEN'],
      validFrom: null,
      validUntil: null,
      // Samengevoegd uit meerdere wetten: noem ze allemaal.
      lawId: selves.length === 1 ? first.lawId : null,
      lawName: selves.length === 1 ? first.lawName : null,
      sourceLaws: selves.map((d) => ({ id: d.lawId, name: d.lawName })),
    });
  }

  for (const d of others) merged.push({ ...d, permissions: orderPermissions(d.permissions) });

  // Mezelf bovenaan, daarna op naam: de eigen zaken zijn het vertrekpunt.
  merged.sort((a, b) => {
    if ((a.subjectType === 'SELF') !== (b.subjectType === 'SELF')) return a.subjectType === 'SELF' ? -1 : 1;
    return String(a.subjectName).localeCompare(String(b.subjectName));
  });

  return { delegations: merged, errors };
}

/**
 * Hoe de machtiging heet in het scherm: 'Ouderlijk gezag', 'Eigenaar'. Een
 * soort die geen label heeft valt terug op de code uit de wet, want die is
 * altijd beter dan niets.
 */
export function delegationLabel(delegation: Pick<Delegation, 'delegationType'> | null | undefined): string | null {
  if (!delegation) return null;
  const type = delegation.delegationType;
  if (!type) return null;
  return delegationTypeLabel(type);
}

/** Mag er met deze machtiging gecorrigeerd en aangevraagd worden? */
export function maySubmitClaims(delegation: Pick<Delegation, 'permissions'> | null | undefined): boolean {
  if (!delegation) return true;
  return delegation.permissions.includes('CLAIMS_INDIENEN');
}

/** De sleutel waarmee een machtiging in de opgeslagen staat wordt aangeduid. */
export function delegationKey(delegation: Pick<Delegation, 'subjectType' | 'subjectId'> | null | undefined): string | null {
  return delegation ? `${delegation.subjectType}:${delegation.subjectId}` : null;
}

/**
 * De machtiging waarmee een profiel binnenkomt: de onderneming met het
 * KvK-nummer uit `start_namens`. Claudia's verhaal gaat over haar zaak, dus ze
 * begint namens die zaak; "Mezelf" blijft gewoon te kiezen. Of ze die zaak
 * mag vertegenwoordigen zegt de wet nog steeds: geeft die de machtiging niet,
 * dan wijst de sleutel nergens heen en begint ze als zichzelf.
 */
export function startDelegationKey(profile: Partial<DemoProfileConfig> | null | undefined): string | null {
  return profile?.start_namens ? delegationKey({ subjectType: 'BUSINESS', subjectId: profile.start_namens }) : null;
}

/** The stored choice for "Mezelf": a choice, but not a delegation. */
export const SELF_KEY = 'SELF';

/** The part of a delegation the store's rules read. */
type DelegationLike = Pick<Delegation, 'subjectType' | 'subjectId'>;

/**
 * The key a choice in the toolbar stores. "Mezelf" is not a delegation but
 * its absence, and still a choice: it wins over the profile's start
 * delegation.
 */
export function chosenDelegationKey(delegation: DelegationLike | null | undefined): string | null {
  return !delegation || delegation.subjectType === 'SELF' ? SELF_KEY : delegationKey(delegation);
}

/**
 * The chosen delegation, or null when acting for oneself. A stored choice that
 * no longer exists (another reference date, another profile) silently falls
 * back to acting for oneself: that is the safe side.
 *
 * The start delegation follows from the profile and the law; `enabled` (the
 * DELEGATION flag) only governs what can be chosen in the toolbar. Without the
 * flag an earlier choice does not count: whoever chose "Mezelf" with the flag
 * on and then turned it off starts on behalf of the business again.
 *
 * `delegations` is everything the law grants, regardless of the flag.
 */
export function activeDelegationFor<D extends DelegationLike>({
  profile,
  chosenKey,
  enabled,
  delegations,
}: {
  profile: Partial<DemoProfileConfig> | null | undefined;
  chosenKey: string | null;
  enabled: boolean;
  delegations: D[];
}): D | null {
  const start = startDelegationKey(profile);
  const key = (enabled ? chosenKey : null) ?? start;
  if (!key || key === SELF_KEY) return null;
  const pool = key === start || enabled ? delegations : [];
  const found = pool.find((d) => delegationKey(d) === key) ?? null;
  return found && found.subjectType !== 'SELF' ? found : null;
}

/** The parameters identifying the subject a law is evaluated for. */
// A type alias, not an interface, so it stays assignable to the store's
// open-ended `SubjectParams`.
export type SubjectIds = {
  bsn?: string;
  kvk_nummer?: string;
};

/**
 * The parameters a law is asked about the subject with.
 *
 * Acting on behalf of someone else, the parameters are about that other: a
 * business is asked about by its KvK number, a child by its BSN. Whoever acts
 * for themselves is a citizen, even when she has a business: she reaches that
 * through the delegation.
 */
export function subjectParams(
  delegation: DelegationLike | null | undefined,
  profile: Pick<DemoProfileConfig, 'bsn'>,
): SubjectIds {
  if (delegation?.subjectType === 'BUSINESS') return { kvk_nummer: delegation.subjectId };
  if (delegation?.subjectType === 'CITIZEN') return { bsn: delegation.subjectId };
  return { bsn: profile.bsn };
}

/**
 * The BSN this is about now. On behalf of a child that is the child; on behalf
 * of a business it stays the agent's, because a business has none.
 */
export function subjectBsnFor(
  delegation: DelegationLike | null | undefined,
  profile: Pick<DemoProfileConfig, 'bsn'> | null | undefined,
): string | undefined {
  return delegation?.subjectType === 'CITIZEN' ? delegation.subjectId : profile?.bsn;
}

/** Which kind of schemes the portal shows: those of the subject. */
export function portalSubject(delegation: Pick<Delegation, 'subjectType'> | null | undefined): Subject {
  return delegation?.subjectType === 'BUSINESS' ? 'BUSINESS' : 'CITIZEN';
}

/** The part of a case `caseOfSubject` matches on. */
export interface SubjectCase {
  lawId: string;
  status: string;
  bsn?: string;
  kvk: string | null;
}

/**
 * The subject's case for this law.
 *
 * What is compared follows the law, not the caller: a law that computes on a
 * KvK number has its case on the KvK number, a law about a person its case on
 * the BSN. The distinction matters because an entrepreneur acting for herself
 * carries both: matching on the BSN whenever it was there hid her own business
 * case as soon as an agent had filed it (that case carries the agent's BSN),
 * although the case was about the same business.
 */
export function caseOfSubject<C extends SubjectCase>(
  cases: C[],
  lawEntry: { id: string; doc?: LawDocLike | null },
  params: SubjectIds,
): C | null {
  const onBusiness = subjectOf(lawEntry.doc) === 'BUSINESS' && params.kvk_nummer !== undefined;
  return (
    cases.find(
      (c) =>
        c.lawId === lawEntry.id &&
        c.status !== 'WITHDRAWN' &&
        (onBusiness ? c.kvk === params.kvk_nummer : c.bsn === params.bsn),
    ) ?? null
  );
}
