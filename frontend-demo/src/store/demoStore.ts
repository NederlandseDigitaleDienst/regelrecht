/**
 * Demo state: the active presenter profile, the reference date, the cases the
 * citizen submitted and the corrections (claims) they made. Persisted in
 * localStorage so a page refresh during a presentation loses nothing; the menu
 * has "Reset" to start clean.
 *
 * Also the glue between corpus, engine and views: `useDemo()` returns the
 * loaded corpus, a ready engine with persona data registered, and helpers to
 * evaluate laws and to mutate cases/claims (which re-register the claim source).
 */
import { computed, markRaw, reactive, ref, shallowRef, watch } from 'vue';
import { loadCorpus } from '../data/loadCorpus';
import {
  evaluateLaw as engineEvaluate,
  executeStage as engineExecuteStage,
  prepareEngine,
  registerClaims,
  registerPersonaData,
} from '../engine/useDemoEngine';
import { reviewedByCaseworker, statusOf } from '../data/lifecycle';
import { declaredClaim } from '../data/declarations';
import { delegationKey, delegationTypeLabel, delegationsFor, maySubmitClaims, startDelegationKey } from '../data/delegation';
import { verdictOf } from '../data/format';
import { driftOf } from '../data/caseDrift';
import { isEntrypointFor, subjectOf } from '../data/entrypoints';
import { assignClaimOwnership } from '../data/claimOwnership';
import { activeLocale, t } from '../i18n/index.js';
import type { Corpus, DemoConfig, LawEntry, PersonaProfile, DemoProfileConfig } from '../data/loadCorpus';
import type { DemoEngine, EngineClaim, EvaluateResult, StageState } from '../engine/useDemoEngine';
import type { Drift } from '../data/caseDrift';
import type { Delegation, DelegationError } from '../data/delegation';
import type { DeclaredClaim } from '../data/declarations';

const STORAGE_KEY = 'rr-demo-state-v1';

// ---- types -----------------------------------------------------------------
//
// The shapes below are persisted in localStorage (`STORAGE_KEY`), so their keys
// are data: renaming one loses what a running demo had built up.

/**
 * The parameters a law is evaluated with for a subject: a BSN for a person, a
 * KvK number for a business, plus whatever the application form added.
 */
export interface SubjectParams {
  bsn?: string;
  kvk_nummer?: string;
  [key: string]: unknown;
}

/** Who acted on behalf of whom, recorded on a case or claim made under a delegation. */
export interface ActingOn {
  actorBsn: string | null;
  actorName: string | null;
  subjectId: string;
  subjectName: string;
  subjectType: string;
  delegationType: string | null;
  lawId: string | null;
  lawName: string | null;
}

/**
 * One entry in a case's history. Since events carry a `key` + `vars` they are
 * translated when shown; an event persisted before that carries the Dutch
 * sentence itself in `text`.
 */
export interface CaseEvent {
  at: string;
  /** `SUBMITTED`, `IN_REVIEW`, `DECIDED`, `BEKENDMAKING`, `OBJECTION`, `OBJECTION_DECIDED`, or a status. */
  type: string;
  key?: string;
  vars?: Record<string, unknown>;
  text?: string;
  approved?: boolean;
  upheld?: boolean;
}

/** An objection (bezwaar, Awb 6:4) against a decided case. */
export interface CaseObjection {
  reason: string;
  status: 'PENDING' | 'UPHELD' | 'DISMISSED';
  filedAt: string;
  decidedAt?: string;
}

/**
 * The engine's procedure state (RFC-008) as kept on a case, with the engine's
 * own keys. The store reads `current_stage` and `accumulated_outputs`.
 */
export type CaseStageState = StageState & {
  current_stage?: string | null;
  accumulated_outputs?: Record<string, unknown> | null;
};

/** A case (zaak): an application someone submitted, and what became of it. */
export interface DemoCase {
  id: string;
  /** The subject's BSN; absent for an application on behalf of a business. */
  bsn?: string;
  /** Whose corrections count for this case (the agent's BSN for a business). */
  claimsBsn?: string;
  kvk: string | null;
  lawId: string;
  lawPath: string;
  lawName: string;
  service: string;
  parameters: SubjectParams;
  claimedResult: Record<string, unknown>;
  verifiedResult: Record<string, unknown> | null;
  /** `SUBMITTED`, `IN_REVIEW`, `DECIDED` or `WITHDRAWN`; derived from the stage when there is one. */
  status: string;
  approved: boolean | null;
  /** Absent on a case persisted before reasons were translated. */
  reasonKey?: string | null;
  /** The caseworker's own words; a case persisted before `reasonKey` holds a Dutch sentence here. */
  reason?: string;
  submittedAt: string;
  decidedAt: string | null;
  publishedAt?: string | null;
  objection: CaseObjection | null;
  acting: ActingOn | null;
  events: CaseEvent[];
  stageState?: CaseStageState | null;
  lifecycleInputs?: Record<string, unknown>;
  lifecycleError?: string | null;
  pendingInputs?: string[];
}

/** An uploaded document attached to a claim. */
export interface ClaimEvidence {
  name: string;
  type: string;
  size: number;
  // null for a file too large to inline (EditValueSheet).
  dataUrl?: string | null;
}

/** Who submitted a claim: the citizen from the portal, or the caseworker. */
export type Claimant = 'BURGER' | 'BEHANDELAAR';

/** A claim (correction): someone changed the value of one input of one law. */
export interface DemoClaim {
  id: string;
  /** The BSN the claim belongs to; see `subjectBsn`. */
  bsn?: string;
  /** The law that owns the input. */
  lawId: string;
  /** The law shown on the tile the claim was made from. */
  tileLawId?: string | null;
  input: string;
  keyField: string;
  /** Missing only when it was submitted before a profile was known; see `submitClaim`. */
  keyValue?: string | null;
  oldValue: unknown;
  newValue: unknown;
  reason: string;
  evidence: ClaimEvidence | null;
  hardship: { clause: string } | null;
  claimant: Claimant;
  selfDeclared: boolean;
  status: 'APPROVED' | 'PENDING' | 'REJECTED';
  caseId: string | null;
  acting: ActingOn | null;
  submittedAt: string;
  decidedAt: string | null;
  decisionReason?: string;
}

/** How the presentation runs: with a presenter in a room, or clicked through alone. */
export type PresentationMode = 'zaal' | 'zelfstandig';

/** The persisted demo state. */
export interface DemoState {
  profileKey: string | null;
  referenceDate: string;
  manualReview: boolean;
  autoAnnounce: boolean;
  cases: DemoCase[];
  claims: DemoClaim[];
  presenterName: string;
  presentationMode: PresentationMode;
  delegationKey: string | null;
  featureOverrides: Partial<Record<string, boolean>>;
}

/** An evaluation, as far as the case functions read it (an `EvaluateResult` fits). */
export interface CaseEvaluation {
  ok?: boolean;
  outputs?: Record<string, unknown> | null;
}

/** What `submitClaim` takes. */
export interface SubmitClaimInput {
  lawId: string;
  tileLawId?: string | null;
  input: string;
  keyField: string;
  keyValue?: string | null;
  oldValue?: unknown;
  newValue: unknown;
  reason: string;
  evidence?: ClaimEvidence | null;
  hardship?: { clause: string } | null;
  selfDeclared?: boolean;
  claimant?: Claimant;
  bsn?: string;
  caseId?: string | null;
  approve?: boolean | null;
}

function today(): string {
  // Local calendar date, not UTC: in the evening the two differ.
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

function defaultState(): DemoState {
  return {
    profileKey: null, // null = config default
    referenceDate: today(),
    // Uit, zoals in de POC (`SAMPLE_RATE = 0.0`): een aanvraag waarbij de
    // burger niets gewijzigd heeft, rekent de wet zelf af en wordt direct
    // toegekend. De presentator kan het aanzetten om te laten zien hoe een
    // behandelaar dezelfde zaak ziet.
    manualReview: false,
    // Aan: een besluit gaat meteen de deur uit, zoals bij een geautomatiseerde
    // toekenning in de praktijk. Uit laat de presentator de bekendmaking als
    // eigen stap tonen (Awb 3:41), met de bezwaartermijn die pas dan begint.
    autoAnnounce: true,
    cases: [],
    claims: [],
    presenterName: '',
    // 'zaal' of 'zelfstandig'. Zaal is de standaard: daar staat een presentator
    // voor een publiek en is het scherm van de demo. De dia's met een route
    // tonen dan géén rail ernaast, want die verkleint precies het beeld waar
    // het publiek naar kijkt. Zelfstandig is voor wie zonder presentator
    // doorklikt; dan is de rail juist het verhaal dat ontbreekt.
    presentationMode: 'zaal',
    // Namens wie er gehandeld wordt: null is voor zichzelf. Bewaard als
    // sleutel (`BUSINESS:85234567`), niet als het hele object, want de
    // machtiging zelf komt uit de wet en wordt bij het laden opnieuw bepaald.
    // null = de startmachtiging van het profiel (`start_namens`), 'SELF' =
    // bewust Mezelf gekozen. Een eigen waarde voor Mezelf, want een opgeslagen
    // staat van vóór `start_namens` heeft hier null, en die hoort bij de start.
    delegationKey: null,
    // Vlaggen die de presentator tijdens de demo heeft omgezet. Alleen wat
    // hij écht aanraakte staat hier; de rest volgt het profiel uit
    // demo-config.yaml. Zo blijft "resetten" terug naar de bedoelde opzet.
    featureOverrides: {},
  };
}

function loadState(): DemoState {
  try {
    const raw = window.localStorage?.getItem(STORAGE_KEY);
    if (!raw) return defaultState();
    return { ...defaultState(), ...JSON.parse(raw) };
  } catch {
    return defaultState();
  }
}

const state = reactive<DemoState>(loadState());
/**
 * Whether changes are written to localStorage. Off while a recorded
 * walkthrough plays: it drives this same state through the presenter's
 * session, and that must not overwrite what the viewer had built up.
 */
let persisting = true;
export function setPersistence(on: boolean): void {
  persisting = !!on;
}
watch(
  state,
  (s) => {
    if (!persisting) return;
    try {
      window.localStorage?.setItem(STORAGE_KEY, JSON.stringify(s));
    } catch {
      /* storage unavailable: the demo still works for this session */
    }
  },
  { deep: true },
);

// Shallow on purpose: the corpus holds 80 parsed law documents and the engine
// is a wasm-bindgen object; wrapping either in a deep proxy would be slow and
// break the engine's private pointer access.
const loadedCorpus = shallowRef<Corpus | null>(null);
const engine = shallowRef<DemoEngine | null>(null);

/**
 * Het corpus zoals elk scherm het leest, met de configuratie in de taal die
 * aanstaat.
 *
 * Hier en niet in `loadCorpus`, omdat dit de reactieve kant is: `currentLocale`
 * is een ref, dus een computed eromheen laat elk scherm dat `corpus.config`
 * leest opnieuw tekenen bij een taalwissel. Het corpus zelf wordt niet opnieuw
 * geladen; alleen welke van de configuraties eruit komt verandert.
 *
 * Het Nederlands is de terugval: zonder overlay draait de demo in het
 * Nederlands, en dat is beter dan lege dia's.
 *
 * De identiteitscheck erna is geen optimalisatie achteraf: het corpus zit in
 * een `shallowRef`, dus een vers object per aanroep zou elk scherm dat het
 * leest laten hertekenen. In de bron-taal komt hetzelfde object terug.
 */
const corpus = computed<Corpus | null>(() => {
  const c = loadedCorpus.value;
  if (!c) return null;
  const config = c.configByLocale?.[activeLocale.value] ?? c.config;
  const profiles = c.profilesByLocale?.[activeLocale.value] ?? c.profiles;
  if (config === c.config && profiles === c.profiles) return c;
  return markRaw({ ...c, config, profiles });
});
const ready = ref(false);
const loadError = ref<unknown>(null);
/** Bumped whenever registered data changed; views re-evaluate on it. */
const dataVersion = ref(0);
let bootPromise: Promise<void> | null = null;

function newId(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

function nowIso(): string {
  return new Date().toISOString();
}

/**
 * Cases in the shape the materialiser's `kind: cases` bindings expect: what
 * was decided (the outputs, so precario reads the area the APV granted, not
 * the area applied for), what was asked, and the case's own facts on top.
 */
function casesForMaterialiser() {
  return state.cases.map((c) => ({
    ...(c.verifiedResult ?? c.claimedResult ?? {}),
    ...(c.parameters ?? {}),
    law: c.lawPath,
    service: c.service,
    status: c.status,
    approved: c.approved,
    bsn: c.bsn,
    kvk_nummer: c.kvk ?? null,
    year: Number(c.submittedAt?.slice(0, 4)),
  }));
}

/**
 * The corrections the citizen's own view computes with: everything they have
 * submitted that has not been rejected, so a pending one counts too.
 *
 * That is the POC's behaviour (`web/routers/laws.py` calls the engine with
 * `approved=False` and feeds it every PENDING and APPROVED claim), and it is
 * the point of the portal: someone who corrects their income wants to see what
 * that would mean, not the old amount with an arrow next to it. Nothing is
 * granted by it — the case still waits for a caseworker, who recomputes with
 * approved values only.
 */
function claimsForEngine(): EngineClaim[] {
  return state.claims
    .filter((c) => c.status === 'APPROVED' || c.status === 'PENDING')
    .map((c) => ({
      lawId: c.lawId,
      keyField: c.keyField,
      // Passed on as it was; see `DemoClaim.keyValue`.
      keyValue: c.keyValue as string,
      input: c.input,
      newValue: c.newValue,
    }));
}

function reregister(): void {
  if (!engine.value || !corpus.value) return;
  registerPersonaData(engine.value, corpus.value, state.referenceDate, casesForMaterialiser(), claimsForEngine());
  registerClaims(engine.value, claimsForEngine());
  dataVersion.value += 1;
}

/**
 * Drop persisted cases and claims whose persona no longer exists. The persona
 * BSNs live in profiles.yaml and can change between deploys (they moved into
 * the 999-test range once); a record that survived in localStorage from before
 * such a change refers to data the materialiser cannot produce, so it can never
 * be recomputed. A record is kept when either its BSN is a persona in
 * profiles.yaml or its KvK number belongs to a configured presenter profile.
 */
function pruneStaleRecords(c: Corpus): void {
  const knownBsns = new Set(Object.keys(c.profiles?.profiles ?? {}));
  const knownKvks = new Set(
    Object.values(c.config?.profiles ?? {})
      .map((p) => p.kvk)
      .filter(Boolean),
  );
  const isKnown = (rec: { bsn?: string; kvk?: string | null }) => knownBsns.has(String(rec.bsn)) || (rec.kvk != null && knownKvks.has(String(rec.kvk)));
  const keptCases = state.cases.filter(isKnown);
  const keptClaims = state.claims.filter(isKnown);
  const droppedCases = state.cases.length - keptCases.length;
  const droppedClaims = state.claims.length - keptClaims.length;
  if (droppedCases === 0 && droppedClaims === 0) return;
  state.cases = keptCases;
  state.claims = keptClaims;
  console.info(
    `Demo: ${droppedCases} verouderde zaken en ${droppedClaims} verouderde correcties uit de opgeslagen staat verwijderd (persona bestaat niet meer in profiles.yaml).`,
  );
}

async function boot(): Promise<void> {
  if (bootPromise) return bootPromise;
  bootPromise = (async () => {
    try {
      loadedCorpus.value = markRaw(await loadCorpus());
      // `loadedCorpus` was set on the line above, so the computed is not null.
      pruneStaleRecords(corpus.value!);
      engine.value = markRaw(await prepareEngine(corpus.value!));
      reregister();
      ready.value = true;
    } catch (e) {
      loadError.value = e;
      throw e;
    }
  })();
  return bootPromise;
}

const profileKey = computed<string>(() => state.profileKey ?? corpus.value?.config?.default_profile ?? 'merijn');
const profile = computed<DemoProfileConfig | null>(() => corpus.value?.config?.profiles?.[profileKey.value] ?? null);
const persona = computed<PersonaProfile | null>(() => {
  const p = profile.value;
  if (!p || !corpus.value) return null;
  return corpus.value.profiles.profiles?.[p.bsn] ?? null;
});

// ---- vlaggen ---------------------------------------------------------------

/**
 * De vlaggen die de demo kent, in de volgorde waarin ze in het menu staan.
 *
 * De POC zette deze in omgevingsvariabelen (`FEATURE_*`), dus alleen te
 * wijzigen door de server opnieuw te starten. Hier hoort een presentator ze
 * midden in zijn verhaal aan te kunnen zetten, dus staan ze in het demo-menu.
 */
/**
 * Waarom een zaak met de hand beoordeeld wordt.
 *
 * Een sleutel en geen zin: de gebeurtenissen van een zaak gaan naar
 * localStorage, en een opgeslagen Nederlandse zin zou na een taalwissel
 * Nederlands blijven terwijl de rest van het scherm Engels is. De zaak bewaart
 * dus wát er gebeurde; welke woorden daarbij horen is een vraag van het moment
 * van tonen.
 */
function reviewReasonKey({ pendingClaims, undecided, assessed = false }: { pendingClaims: unknown[]; undecided: boolean; assessed?: boolean }): string {
  if (pendingClaims.length) return 'case.review.citizen_changed_data';
  if (undecided) return 'case.review.law_needs_more_facts';
  if (assessed) return 'case.review.assessed_by_service';
  return 'case.review.sample';
}



/**
 * De zin bij een gebeurtenis, in de taal die aan staat.
 *
 * `key` + `vars` is wat een zaak sinds deze versie bewaart. Een zaak die al in
 * localStorage stond draagt nog een kant-en-klare Nederlandse `text`, en die
 * blijft staan: hem weggooien zou de geschiedenis van een lopende demo wissen,
 * en hem vertalen kan niet, want de woorden zijn het enige wat ervan over is.
 */
export function eventText(event: Partial<CaseEvent> | null | undefined): string {
  if (!event) return '';
  if (event.key) return t(event.key, event.vars);
  return event.text ?? '';
}

/** Waarom een zaak is toegekend of afgewezen, in de taal die aan staat. */
export function caseReason(c: Partial<Pick<DemoCase, 'reasonKey' | 'reason'>> | null | undefined): string | null {
  if (!c) return null;
  if (c.reasonKey) return t(c.reasonKey);
  return c.reason ?? null;
}

/** A feature flag the demo knows. Its label is `app.features.<key>` in the dictionaries. */
export interface FeatureFlag {
  key: FeatureKey;
  icon: string;
  /** What the flag turns on; not shown in the menu. */
  hint: string;
}

export type FeatureKey = 'DELEGATION' | 'CHANGE_WIZARD' | 'HARMONIZE' | 'AUTO_APPROVE_CLAIMS';

export const FEATURES: FeatureFlag[] = [
  // Het label staat niet hier maar in de woordenboeken, onder
  // `app.features.<key>`: de demo is tweetalig, en een Nederlands label hier
  // zou ernaast blijven staan alsof het nog iets aanstuurde.
  //
  // `hint` beschrijft wat de vlag aanzet en staat niet in het menu: het
  // `details`-attribuut van nldd-menu-item is een kort label rechts, en een
  // hele zin daarin perst het label op een smal scherm in een kolom van één
  // woord breed. Het blijft hier staan als uitleg bij de vlag zelf.
  { key: 'DELEGATION', icon: 'switch', hint: 'Handelen namens een kind of een onderneming' },
  { key: 'CHANGE_WIZARD', icon: 'edit', hint: 'Eén ingang voor inkomen, huur, adres en huishouden' },
  { key: 'HARMONIZE', icon: 'chart-x-y-axis-line', hint: 'Eén staffel, op het simulatietabblad' },
  // Geen vinkje-achtig icoon: het menu-item zet er zelf al een vinkje voor als
  // de vlag aan staat, en twee vinkjes naast elkaar leest als een fout.
  { key: 'AUTO_APPROVE_CLAIMS', icon: 'lightning', hint: 'Zonder tussenkomst van een behandelaar' },
];

/**
 * Staat een vlag aan? Wat de presentator omzette wint van het profiel; wat hij
 * niet aanraakte volgt `demo-config.yaml`. Alles leest via deze ene plek, want
 * een schakelaar die maar de helft van de features bereikt is erger dan geen
 * schakelaar.
 */
function featureEnabled(key: string): boolean {
  const override = state.featureOverrides?.[key];
  if (override !== undefined) return override;
  return !!profile.value?.feature_flags?.[key];
}

/** Reactieve vorm van `featureEnabled`, voor gebruik in een template. */
// Every key of FEATURES is mapped, so the record is complete.
const features = computed(() => Object.fromEntries(FEATURES.map((f) => [f.key, featureEnabled(f.key)])) as Record<FeatureKey, boolean>);

function toggleFeature(key: string): void {
  state.featureOverrides = { ...state.featureOverrides, [key]: !featureEnabled(key) };
}

/** Terug naar wat het profiel zegt, voor alle vlaggen. */
function resetFeatures(): void {
  state.featureOverrides = {};
}

// ---- machtigingen ----------------------------------------------------------

/**
 * Namens wie de ingelogde persoon mag handelen, volgens de wet. Elke
 * provider-wet wordt met de engine geëvalueerd, dus dit verandert mee met de
 * peildatum en met gecorrigeerde gegevens; `dataVersion` triggert dat.
 */
const delegationResult = computed<{ delegations: Delegation[]; errors: DelegationError[] }>(() => {
  // Lees `dataVersion` zodat een correctie of een nieuwe peildatum doorwerkt.
  dataVersion.value; // eslint-disable-line no-unused-expressions
  if (!ready.value || !profile.value?.bsn) return { delegations: [], errors: [] };
  return delegationsFor(engine.value, corpus.value, profile.value.bsn, state.referenceDate);
});

/** Of dit profiel machtigingen mag gebruiken (demo-config per profiel). */
const delegationEnabled = computed(() => features.value.DELEGATION);

/** De machtigingen die dit profiel kan kiezen; leeg als de vlag uit staat. */
const delegations = computed<Delegation[]>(() => (delegationEnabled.value ? delegationResult.value.delegations : []));

/**
 * De gekozen machtiging, of null als er voor zichzelf gehandeld wordt.
 * Een bewaarde keuze die niet meer bestaat (andere peildatum, ander profiel)
 * vervalt stil naar 'voor zichzelf': dat is de veilige kant.
 */
const activeDelegation = computed<Delegation | null>(() => {
  const start = startDelegationKey(profile.value);
  // De startmachtiging volgt uit het profiel en de wet; de vlag gaat alleen
  // over wat er in de werkbalk te kiezen is. Zonder de vlag telt een eerder
  // gemaakte keuze dus niet: wie met de vlag aan "Mezelf" koos en hem daarna
  // uitzette, begint weer namens de zaak.
  const key = (delegationEnabled.value ? state.delegationKey : null) ?? start;
  if (!key || key === SELF_KEY) return null;
  const pool = key === start ? delegationResult.value.delegations : delegations.value;
  const found = pool.find((d) => delegationKey(d) === key) ?? null;
  return found && found.subjectType !== 'SELF' ? found : null;
});

/** Mag er in de huidige context gecorrigeerd en aangevraagd worden? */
const canSubmitClaims = computed(() => maySubmitClaims(activeDelegation.value));

/**
 * De BSN waar het nu over gaat. Namens een kind is dat het kind; namens een
 * onderneming blijft het de gemachtigde, want een onderneming heeft er geen.
 */
function subjectBsn(): string | undefined {
  const d = activeDelegation.value;
  return d?.subjectType === 'CITIZEN' ? d.subjectId : profile.value?.bsn;
}

/** De opgeslagen keuze voor Mezelf (zie `delegationKey` in defaultState). */
const SELF_KEY = 'SELF';

function setDelegation(delegation: Delegation | null): void {
  const key = delegationKey(delegation);
  // 'Mezelf' is geen machtiging maar de afwezigheid ervan, en wel een keuze:
  // die wint van de startmachtiging van het profiel.
  state.delegationKey = !delegation || delegation.subjectType === 'SELF' ? SELF_KEY : key;
}

/**
 * Parameters the portal passes to a law for the active persona.
 *
 * Handelt iemand namens een ander, dan gaan de parameters over die ander: een
 * onderneming wordt op haar KvK-nummer bevraagd, een kind op zijn BSN. Dat is
 * het hele punt van machtigen — de wet rekent over het onderwerp, niet over
 * degene die de knop indrukt. Wie voor zichzelf handelt is een burger, ook als
 * zij een onderneming heeft: die bereikt ze via de machtiging.
 */
function personaParams(): SubjectParams {
  const d = activeDelegation.value;
  if (d?.subjectType === 'BUSINESS') return { kvk_nummer: d.subjectId };
  if (d?.subjectType === 'CITIZEN') return { bsn: d.subjectId };
  // Throws without a profile, as it did in JavaScript; callers run after boot.
  return { bsn: profile.value!.bsn };
}

function setProfile(key: string | null): void {
  state.profileKey = key;
  // De machtigingen van het vorige profiel gelden niet voor dit profiel; dit
  // profiel begint weer bij zijn eigen startmachtiging.
  state.delegationKey = null;
}

function setReferenceDate(date: string): void {
  state.referenceDate = date;
  reregister();
}

/**
 * Is a law shown on the active profile's portal?
 *
 * `hidden_laws` geldt altijd: dat zijn infrastructuurwetten die nergens op een
 * portaal horen. `disabled_laws` is iets anders — het snoeit het portaal van
 * één persona bij, zodat het verhaal van díe persoon overzichtelijk blijft.
 * Zodra iemand namens een ander handelt gaat dat niet meer op: de regelingen
 * van een onderneming zijn niet weggelaten omdat de gemachtigde ze in zijn
 * eigen portaal niet wil zien. Zonder deze uitzondering staat het portaal
 * namens Merijns eigen bedrijf helemaal leeg.
 */
function isLawEnabled(lawEntry: Pick<LawEntry, 'service' | 'law_path'>): boolean {
  const cfg: DemoConfig = corpus.value?.config ?? {};
  const inList = (map: Record<string, string[] | undefined> | undefined) => (map?.[lawEntry.service] ?? []).some((p) => lawEntry.law_path === p || lawEntry.law_path.startsWith(`${p}/`));
  if (inList(cfg.hidden_laws)) return false;
  if (!activeDelegation.value && inList(profile.value?.disabled_laws)) return false;
  return true;
}

/**
 * Laws the active persona can discover on their portal.
 *
 * Namens een onderneming zijn dat de ondernemersregelingen, namens een kind de
 * burgerregelingen: waar de wet over gaat volgt het onderwerp, niet degene die
 * inlogt. Voor zichzelf ziet ook een ondernemer de burgerregelingen. Het
 * profiel bepaalt nog wel wat verborgen blijft, want dat is een keuze van de
 * demo en niet van de wet.
 */
const portalLaws = computed<LawEntry[]>(() => {
  if (!corpus.value || !profile.value) return [];
  const d = activeDelegation.value;
  const wanted = d?.subjectType === 'BUSINESS' ? 'BUSINESS' : 'CITIZEN';
  return [...corpus.value.latestById.values()].filter(
    (law) => isEntrypointFor(law.doc, wanted) && isLawEnabled(law),
  );
});

function evaluate(
  lawEntry: { id: string; outputs: string[] },
  params: SubjectParams = personaParams(),
  outputs: string[] | null = null,
): EvaluateResult {
  // Before boot the engine is null; evaluateLaw catches the resulting
  // TypeError and returns `ok: false`, as it did in JavaScript.
  return engineEvaluate(engine.value as DemoEngine, lawEntry, params, state.referenceDate, outputs);
}

// ---- cases -----------------------------------------------------------------

/**
 * De zaak van het huidige onderwerp voor deze wet.
 *
 * Waarop vergeleken wordt volgt de wet, niet de aanroeper: een wet die om een
 * KvK-nummer rekent heeft een zaak op het KvK-nummer, een
 * wet over een persoon een zaak op de BSN. Dat onderscheid is nodig omdat een
 * ondernemer bij zichzelf allebei bij zich draagt: matchen op de BSN zolang
 * die er is, liet een ondernemer zijn eigen bedrijfszaak niet zien zodra een
 * gemachtigde die had ingediend (die zaak staat op diéns BSN), terwijl de
 * zaak wel over dezelfde onderneming ging.
 */
function findCase(lawEntry: Pick<LawEntry, 'id' | 'doc'>, subject: SubjectParams | null = null): DemoCase | null {
  const params = subject ?? personaParams();
  const onBusiness = subjectOf(lawEntry.doc) === 'BUSINESS' && params.kvk_nummer !== undefined;
  return (
    state.cases.find(
      (c) =>
        c.lawId === lawEntry.id &&
        c.status !== 'WITHDRAWN' &&
        (onBusiness ? c.kvk === params.kvk_nummer : c.bsn === params.bsn),
    ) ?? null
  );
}

/**
 * Wie de handeling verricht, als het niet de persoon zelf is. Komt in het
 * dossier terecht: een besluit dat namens een ander is aangevraagd moet
 * terug te vinden zijn bij wie dat deed.
 */
function actingOn(): ActingOn | null {
  const d = activeDelegation.value;
  if (!d) return null;
  return {
    actorBsn: profile.value?.bsn ?? null,
    actorName: profile.value?.name ?? null,
    subjectId: d.subjectId,
    subjectName: d.subjectName,
    subjectType: d.subjectType,
    delegationType: d.delegationType,
    lawId: d.lawId,
    lawName: d.lawName,
  };
}

/**
 * Submit an application. Goes to manual review when the citizen changed data
 * for this law, when the service always assesses it (`review_laws`, such as
 * Rotterdam's terrasvergunning) or when the demo runs in "alles handmatig
 * beoordelen" mode; otherwise the decision follows the law's outcome directly.
 */
function submitCase(lawEntry: LawEntry, evaluation: CaseEvaluation, params: SubjectParams = personaParams()): DemoCase {
  const bsn = params.bsn;
  // Namens een onderneming heeft de aanvraag geen BSN in haar parameters, maar
  // de correcties van de gemachtigde staan wel op diens BSN (subjectBsn). Zonder
  // die terugval vond de zaak ze niet en werd ze op ongecontroleerde gegevens
  // automatisch toegekend. De zaak zelf houdt het BSN van haar onderwerp: wie
  // daarna voor zichzelf kijkt, ziet de zaak van de BV niet als de zijne.
  const claimsBsn = bsn ?? subjectBsn();
  const acting = actingOn();
  const pendingClaims = state.claims.filter(
    (c) => c.bsn === claimsBsn && c.status === 'PENDING' && c.tileLawId === lawEntry.id,
  );
  // An unknown verdict (facts missing, RFC-036) is not a yes: the application
  // goes to a caseworker, who completes it (Awb art. 4:5) or decides.
  const verdict = verdictOf(evaluation.outputs);
  const undecided = verdict === 'unknown';
  const requirementsMet = verdict === null || verdict === true;
  const assessed = reviewedByCaseworker(lawEntry, corpus.value?.config);
  const needsReview = state.manualReview || assessed || pendingClaims.length > 0 || undecided;
  const c: DemoCase = {
    id: newId('zaak'),
    bsn,
    claimsBsn,
    kvk: params.kvk_nummer ?? null,
    lawId: lawEntry.id,
    lawPath: lawEntry.law_path,
    lawName: lawEntry.name,
    service: lawEntry.service,
    parameters: params,
    claimedResult: evaluation.outputs ?? {},
    verifiedResult: null,
    status: needsReview ? 'IN_REVIEW' : 'DECIDED',
    approved: needsReview ? null : requirementsMet,
    reasonKey: needsReview ? null : requirementsMet ? 'case.reason.granted_by_law' : 'case.reason.conditions_not_met',
    submittedAt: nowIso(),
    decidedAt: needsReview ? null : nowIso(),
    objection: null,
    // Namens wie deze aanvraag is ingediend, als dat niet de persoon zelf was.
    acting,
    events: [
      {
        at: nowIso(),
        type: 'SUBMITTED',
        ...(acting
          ? {
              key: 'case.event.submitted_by_agent',
              vars: {
                actor: acting.actorName,
                subject: acting.subjectName,
                kind: delegationTypeLabel(acting.delegationType),
              },
            }
          : { key: 'case.event.submitted' }),
      },
      needsReview
        ? { at: nowIso(), type: 'IN_REVIEW', key: reviewReasonKey({ pendingClaims, undecided, assessed }) }
        : { at: nowIso(), type: 'DECIDED', key: requirementsMet ? 'case.event.granted_auto' : 'case.event.refused_auto' },
    ],
  };
  for (const claim of pendingClaims) claim.caseId = c.id;
  state.cases.unshift(c);
  // De levensloop begint. De aanvraag is er, dus AANVRAAG en BEHANDELING
  // hebben hun datums; daarna wacht de engine op de besluitdatum, die er pas
  // is als er besloten wordt. Bij een aanvraag die meteen wordt toegekend,
  // volgt die in dezelfde adem.
  const vandaag = isoDate(c.submittedAt);
  advanceLifecycle(c, { aanvraag_datum: vandaag, beslistermijn_start: vandaag });
  if (!needsReview) {
    advanceLifecycle(c, { besluit_datum: vandaag });
  }
  c.status = statusOf(c);
  announceIfAutomatic(c);
  reregister();
  return c;
}

/**
 * Een genomen besluit meteen bekendmaken, als de presentator dat zo heeft
 * staan. Bekendmaken blijft een eigen handeling (publishCase); deze schakelaar
 * slaat alleen de knop over.
 */
function announceIfAutomatic(c: DemoCase): void {
  if (state.autoAnnounce && statusOf(c) === 'DECIDED' && !c.publishedAt) publishCase(c.id);
}

/**
 * De levensloop van een zaak een stuk verder brengen met de gegevens die er nu
 * zijn (RFC-008).
 *
 * De engine loopt de fasen af die de Awb aan dit soort besluit geeft, vuurt bij
 * elke fase de haken die daarbij horen, en stopt zodra hij een gegeven mist.
 * Wat hij mist staat in `pendingInputs`; de demo levert dat aan op het moment
 * dat het bestaat — een besluitdatum als de behandelaar besluit, een
 * bekendmakingsdatum als het besluit wordt verstuurd.
 *
 * De uitkomsten van de Awb (de motiveringsplicht, de termijn, de einddatum)
 * stapelen zich op in `accumulated_outputs` en worden bij de zaak bewaard. De
 * engine houdt zelf niets vast; dat is hier met opzet de taak van de demo.
 *
 * Een wet die in geen procedure zit, komt in één keer klaar. Dan blijft er geen
 * `stageState` staan en valt alles terug op de gewone status.
 */
function advanceLifecycle(c: DemoCase, supplied: Record<string, unknown> = {}): void {
  const lawEntry = corpus.value?.lawById(c.lawId);
  if (!engine.value || !lawEntry) return;
  // Het uitvoerveld waar deze zaak over gaat: hetzelfde veld waarop de tegel
  // en de aanvraag rekenen.
  const outputName = primaryOutputFor(lawEntry);
  if (!outputName) return;

  const params = { ...(c.parameters ?? {}), ...(c.lifecycleInputs ?? {}), ...supplied };
  c.lifecycleInputs = { ...(c.lifecycleInputs ?? {}), ...supplied };

  const step = engineExecuteStage(engine.value, lawEntry, outputName, c.stageState ?? null, params, state.referenceDate);
  if (!step.ok) {
    // Een levensloop die niet rekent, mag de zaak niet stukmaken: de zaak
    // houdt wat ze had en de fout is te zien waar de uitkomsten staan.
    c.lifecycleError = step.error;
    return;
  }
  c.lifecycleError = null;
  if (step.complete) {
    // Alle fasen doorlopen. De laatste stand blijft staan, zodat de termijn
    // die eruit kwam zichtbaar blijft.
    c.stageState = {
      ...(c.stageState ?? {}),
      current_stage: 'BEZWAAR',
      accumulated_outputs: { ...(c.stageState?.accumulated_outputs ?? {}), ...step.outputs },
    };
    c.pendingInputs = [];
    return;
  }
  // The engine's state carries `current_stage` and `accumulated_outputs` with these types.
  c.stageState = step.state as CaseStageState | null;
  c.pendingInputs = step.pendingInputs;
}

/**
 * Het uitvoerveld dat deze wet voor de burger beantwoordt: hetzelfde veld dat
 * de tegel groot laat zien (`dashboard_outputs`), en anders het eerste dat de
 * wet declareert.
 */
function primaryOutputFor(lawEntry: Pick<LawEntry, 'service' | 'law_path' | 'outputs'>): string | null {
  const configured = corpus.value?.config?.dashboard_outputs?.[`${lawEntry.service}/${lawEntry.law_path}`];
  if (configured) return configured;
  return lawEntry.outputs?.[0] ?? null;
}

/**
 * De burger dient dezelfde aanvraag opnieuw in, met de gegevens zoals ze nu
 * zijn. Dat is de POC's `Case.reset`: geen tweede zaak naast de eerste, maar
 * dezelfde zaak die opnieuw door dezelfde beoordeling gaat, met het verloop
 * dat eraan vastzit intact.
 *
 * Dit is de weg terug uit `caseDrift`. Dat een gewijzigde aanvraag daarna
 * vrijwel altijd bij een behandelaar terechtkomt, is geen keuze van de demo:
 * er ligt een wijziging die nog niet is goedgekeurd, en daar hoort iemand
 * naar te kijken.
 */
function resubmitCase(
  caseId: string,
  evaluation: CaseEvaluation | null | undefined,
  params: SubjectParams = personaParams(),
): DemoCase | null {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c || !evaluation?.ok) return null;
  const verdict = verdictOf(evaluation.outputs);
  const undecided = verdict === 'unknown';
  const requirementsMet = verdict === null || verdict === true;
  // Per BSN, niet per wet: de wijziging die deze zaak raakt kan bij een
  // andere regeling zijn opgegeven. Dat is precies het geval waar dit voor is.
  const pendingClaims = state.claims.filter((cl) => cl.bsn === (c.claimsBsn ?? c.bsn) && cl.status === 'PENDING');
  const assessed = reviewedByCaseworker({ service: c.service, law_path: c.lawPath }, corpus.value?.config);
  const needsReview = state.manualReview || assessed || pendingClaims.length > 0 || undecided;
  c.parameters = params;
  c.claimedResult = evaluation.outputs ?? {};
  c.verifiedResult = null;
  c.approved = needsReview ? null : requirementsMet;
  c.reasonKey = needsReview ? null : requirementsMet ? 'case.reason.granted_by_law' : 'case.reason.conditions_not_met';
  c.decidedAt = needsReview ? null : nowIso();
  c.events.push({ at: nowIso(), type: 'SUBMITTED', key: 'case.event.amended' });
  c.events.push(
    needsReview
      ? { at: nowIso(), type: 'IN_REVIEW', key: reviewReasonKey({ pendingClaims, undecided, assessed }) }
      : { at: nowIso(), type: 'DECIDED', key: requirementsMet ? 'case.event.granted_auto' : 'case.event.refused_auto' },
  );
  // De lijst hierboven is met opzet breder dan deze regeling — een wijziging
  // bij een andere regeling telt mee voor de vraag óf er beoordeeld moet
  // worden — maar eigenaarschap is iets anders dan aanleiding. Zie
  // `assignClaimOwnership`.
  assignClaimOwnership(pendingClaims, c);
  // De levensloop begint opnieuw: dit is dezelfde zaak die nog eens door
  // dezelfde procedure gaat, dus ook het besluit en de bekendmaking worden
  // opnieuw gedaan. De bezwaartermijn die bij het vorige besluit hoorde geldt
  // niet meer voor dit besluit, en hoort dus niet te blijven staan.
  c.stageState = null;
  c.lifecycleInputs = {};
  c.publishedAt = null;
  const opnieuw = isoDate(nowIso());
  advanceLifecycle(c, { aanvraag_datum: opnieuw, beslistermijn_start: opnieuw });
  if (!needsReview) advanceLifecycle(c, { besluit_datum: opnieuw });
  c.status = statusOf(c);
  announceIfAutomatic(c);
  reregister();
  return c;
}

function decideCase(caseId: string, approved: boolean, reason: string, verifiedResult: Record<string, unknown> | null = null): void {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c) return;
  c.approved = approved;
  c.reason = reason;
  c.verifiedResult = verifiedResult;
  c.decidedAt = nowIso();
  c.events.push({
    at: nowIso(),
    type: 'DECIDED',
    approved,
    key: approved ? 'case.event.granted_by_officer' : 'case.event.refused_by_officer',
    vars: { reason },
  });
  // Het besluit is genomen: dat is de datum waar de fase BESLUIT op wachtte.
  advanceLifecycle(c, { besluit_datum: isoDate(c.decidedAt) });
  c.status = statusOf(c);
  announceIfAutomatic(c);
  reregister();
}

/**
 * Het besluit wordt bekendgemaakt (Awb 3:41): het gaat de deur uit naar de
 * belanghebbende.
 *
 * Een eigen handeling en geen bijzaak van het besluit, omdat de Awb er twee
 * verschillende momenten van maakt. RFC-008 zet de overgang BESLUIT →
 * BEKENDMAKING dan ook op "manual", en pas hier vuurt artikel 6:8 en komt de
 * bezwaartermijn als datum tevoorschijn. Vóór dit moment is er niets om
 * bezwaar tegen te maken: de burger weet van niets.
 */
function publishCase(caseId: string, bekendmakingDatum: string | null = null): void {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c || c.publishedAt) return;
  // Bekendmaken is het meedelen van een besluit (Awb 3:41), dus zonder besluit
  // is er niets mee te delen. De knop staat er ook niet eerder, maar dat is een
  // regel van de wet en hoort niet van het scherm af te hangen.
  if (statusOf(c) !== 'DECIDED') return;
  c.publishedAt = nowIso();
  const datum = bekendmakingDatum ?? isoDate(c.publishedAt);
  c.events.push({ at: nowIso(), type: 'BEKENDMAKING', key: 'case.event.announced', vars: { date: datum } });
  advanceLifecycle(c, { bekendmaking_datum: datum });
  c.status = statusOf(c);
  reregister();
}

/** De kalenderdatum uit een tijdstempel; de wet rekent in dagen, niet in uren. */
function isoDate(iso: string | null | undefined): string {
  return String(iso ?? nowIso()).slice(0, 10);
}

function moveCase(caseId: string, status: string): void {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c || c.status === status) return;
  c.status = status;
  c.events.push({ at: nowIso(), type: status, key: 'case.event.status_changed', vars: { status } });
  reregister();
}

/**
 * De burger maakt bezwaar (Awb 6:4).
 *
 * Het bezwaar is een veld op het besluit en niet een stap terug in de
 * levensloop: het besluit blíjft genomen en bekendgemaakt, er is alleen iets
 * tegen ingebracht. Dat de behandeling van een bezwaar strikt genomen een eigen
 * besluit is met een eigen levensloop (Awb 7:12, RFC-008 vraag 2), gaat verder
 * dan wat de demo laat zien; hier is het één veld.
 */
function objectToCase(caseId: string, reason: string): void {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c) return;
  c.objection = { reason, status: 'PENDING', filedAt: nowIso() };
  c.events.push({ at: nowIso(), type: 'OBJECTION', key: 'case.event.objection', vars: { reason } });
  reregister();
}

function decideObjection(caseId: string, upheld: boolean, reason: string): void {
  const c = state.cases.find((x) => x.id === caseId);
  if (!c?.objection) return;
  c.objection.status = upheld ? 'UPHELD' : 'DISMISSED';
  c.objection.decidedAt = nowIso();
  c.status = 'DECIDED';
  if (upheld) c.approved = !c.approved;
  c.events.push({
    at: nowIso(),
    type: 'OBJECTION_DECIDED',
    upheld,
    key: upheld ? 'case.event.objection_upheld' : 'case.event.objection_dismissed',
    vars: { reason },
  });
  reregister();
}

/**
 * Wat een lopende aanvraag nog waard is, gegeven wat er ná het indienen is
 * gewijzigd. De vergelijking zelf staat in `data/caseDrift.ts`; hier wordt
 * alleen de zaak erbij gezocht.
 */
function caseDrift(
  lawEntry: Pick<LawEntry, 'id' | 'doc'>,
  evaluation: CaseEvaluation | null | undefined,
  subject: DemoCase | SubjectParams | null = null,
): Drift<DemoCase> | null {
  // De behandelaar heeft de zaak al in handen en geeft hem mee; de portal
  // zoekt hem op bij het huidige onderwerp.
  // A subject with an `id` is a case; persona parameters have none.
  return driftOf(subject?.id ? (subject as DemoCase) : findCase(lawEntry, subject as SubjectParams | null), evaluation);
}

/**
 * De waarde die nu geldt voor één gegeven van één wet, zónder de correcties
 * die er al liggen.
 *
 * Nodig om bij een nieuwe correctie vast te leggen wat er stond. Dat kan niet
 * uit de gewone doorrekening komen: die telt openstaande correcties mee
 * (`claimsForEngine`), dus daar staat de nieuwe waarde al in en zou "oud"
 * hetzelfde zijn als "nieuw". De correctiebron gaat er daarom even af en
 * daarna weer op.
 *
 * `keyField`/`keyValue` zeggen over wie het gaat: een BSN, een KvK-nummer, een
 * organisatie. Die komen van de correctie zelf, want het onderwerp van een
 * correctie is niet altijd degene die haar indient.
 *
 * `null` als het niet lukt — een wet die niet doorrekent mag een correctie niet
 * tegenhouden; er is dan alleen niets om doorgestreept te tonen.
 */
function currentValueOf(lawId: string, input: string, keyField: string, keyValue: string | null | undefined): unknown {
  const lawEntry = corpus.value?.lawById(lawId);
  if (!engine.value || !lawEntry || !keyField || keyValue == null) return null;
  try {
    registerClaims(engine.value, []);
    // Doorrekenen voor het onderwerp waar de correctie over gáát, en niet voor
    // wie haar indient. Een correctie op een gegeven van een onderneming staat
    // op het KvK-nummer; die voor de BSN van de gemachtigde doorrekenen levert
    // de waarde van een ander op, of een parameter die niet past.
    const result = evaluate(lawEntry, { [keyField]: keyValue }, [input]);
    return result.ok ? result.outputs?.[input] ?? null : null;
  } catch {
    return null;
  } finally {
    // Altijd terugzetten: zonder dit rekent de rest van de demo verder zonder
    // de correcties van de burger.
    registerClaims(engine.value, claimsForEngine());
  }
}

// ---- claims ----------------------------------------------------------------

/**
 * Someone corrects a value. `lawId` is the law that owns the input (which may
 * be a dependency of the tile's law), `tileLawId` the law shown on the tile.
 *
 * By default this is the citizen (the active profile) correcting a register
 * value from the portal. The caseworker corrects the same values from the case
 * inspector: then `claimant` is 'BEHANDELAAR', `bsn` and `caseId` are the
 * case's, and `approve` is true, because the caseworker is the one who would
 * otherwise approve it.
 *
 * Handelt iemand namens een kind, dan hoort de correctie bij dat kind: het
 * gaat om diens gegevens. Namens een onderneming blijft de correctie op het
 * KvK-nummer staan (`keyField`/`keyValue` dragen dat al), en is de BSN van de
 * gemachtigde alleen wie het deed.
 *
 * `evidence` is `{ name, type, size, dataUrl? }` for an uploaded document;
 * `hardship` is `{ clause }` when the correction appeals to a hardship clause.
 */
function submitClaim({
  lawId,
  tileLawId,
  input,
  keyField,
  keyValue,
  oldValue,
  newValue,
  reason,
  evidence = null,
  hardship = null,
  selfDeclared = false,
  claimant = 'BURGER',
  bsn = subjectBsn(),
  caseId = null,
  approve = null,
}: SubmitClaimInput): DemoClaim {
  const acting = claimant === 'BEHANDELAAR' ? null : actingOn();
  // Wat er stond vóór deze correctie. De aanroeper weet dat niet altijd — de
  // wijzigingswizard kent alleen wat de burger invult — en zonder die waarde
  // valt er later niets te tonen dan "0 → 0". Ze wordt hier uitgerekend, bij de
  // engine, en niet in elk scherm apart. Al bestaande correcties tellen niet
  // mee: de oude waarde is wat er stond, niet wat een eerdere correctie er al
  // van gemaakt had.
  const oldValueResolved = oldValue ?? currentValueOf(lawId, input, keyField, keyValue);
  // A value no register holds is the citizen's own declaration and applies at
  // once; a correction of a register value waits for the caseworker unless the
  // profile auto-approves. An appeal to a hardship clause always needs a human,
  // whatever the profile says. An explicit `approve` (the caseworker) wins.
  const autoApprove = approve ?? (hardship ? false : selfDeclared || featureEnabled('AUTO_APPROVE_CLAIMS'));
  const claim: DemoClaim = {
    id: newId('claim'),
    bsn,
    lawId,
    tileLawId,
    input,
    keyField,
    keyValue,
    oldValue: oldValueResolved,
    newValue,
    reason,
    evidence,
    hardship,
    claimant,
    selfDeclared,
    status: autoApprove ? 'APPROVED' : 'PENDING',
    caseId,
    // Namens wie deze correctie is ingediend, als dat niet de persoon zelf was.
    acting,
    submittedAt: nowIso(),
    decidedAt: autoApprove ? nowIso() : null,
  };
  state.claims.unshift(claim);
  reregister();
  return claim;
}

function decideClaim(claimId: string, approved: boolean, reason = ''): void {
  const claim = state.claims.find((c) => c.id === claimId);
  if (!claim) return;
  claim.status = approved ? 'APPROVED' : 'REJECTED';
  claim.decisionReason = reason;
  claim.decidedAt = nowIso();
  reregister();
}

function claimFor(lawId: string, input: string, bsn: string | undefined = subjectBsn()): DemoClaim | DeclaredClaim | null {
  return (
    state.claims.find((c) => c.lawId === lawId && c.input === input && c.bsn === bsn && c.status !== 'REJECTED') ??
    // Wat het profiel al eerder opgaf, zoals Café Noon in zijn accijnsaangifte.
    declaredClaim(profile.value, lawId, input)
  );
}

function resetState(): void {
  Object.assign(state, defaultState());
  reregister();
}

/** What `useDemo()` returns: the store's state, the corpus and engine, and the actions. */
export type DemoStore = ReturnType<typeof useDemo>;

export function useDemo() {
  return {
    state,
    corpus,
    engine,
    ready,
    loadError,
    dataVersion,
    boot,
    profileKey,
    profile,
    persona,
    personaParams,
    features,
    toggleFeature,
    resetFeatures,
    delegations,
    delegationEnabled,
    delegationErrors: computed(() => delegationResult.value.errors),
    activeDelegation,
    canSubmitClaims,
    setDelegation,
    subjectBsn,
    setProfile,
    setReferenceDate,
    reregister,
    portalLaws,
    isLawEnabled,
    evaluate,
    findCase,
    caseDrift,
    submitCase,
    resubmitCase,
    decideCase,
    publishCase,
    moveCase,
    objectToCase,
    decideObjection,
    submitClaim,
    decideClaim,
    claimFor,
    resetState,
  };
}
