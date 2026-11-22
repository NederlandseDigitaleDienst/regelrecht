// The shapes the server in packages/wetsproces answers with. Their keys are
// those of the kern of the chronolexografie poc (`velden`, `bevindingen`,
// `op`), as the API passes them on; only what the screens read is typed.

/** A field of a form, as the kern derives it from the law (`kern::veld`). */
export interface Veld {
  naam: string;
  /** The parameter type: `boolean`, `string`, `date`, `number`, `amount`, `array`, ... */
  type: string | null;
  mag_leeg?: boolean;
  /** The article that asks it, `<regelwerk>#<article>`. */
  basis?: string;
  /** For a table: its columns. */
  kolommen?: { naam: string; type: string }[];
  /** Set on a finding a policy leaves to the authority for this case. */
  beleid?: boolean;
}

/** How a screen names a field, where the UI spec says so (`label`, `help`, `group`). */
export interface Veldtekst {
  label?: string;
  help?: string;
  /** The heading of the group the field stands in. */
  group?: string;
}

/**
 * Where the system takes the value of a field from (`kern::door_het_systeem`):
 * the login, the clock, a fixed value, the decision the law names, the means
 * of login, or a rule (`bron`).
 */
export type Systeemsoort = 'inloggen' | 'datum' | 'waarde' | 'gevraagd_besluit' | 'inlogmiddel' | 'bron';

/** How the system fills a field of the application (`fill: {by: system}`). */
export interface Systeemvulling {
  soort?: Systeemsoort;
  /**
   * The value: one the system knows itself, or for a citizen the value the
   * server will record for them, whatever the kind.
   */
  waarde?: unknown;
  /** The rule whose outcome fills it, `<regelwerk>#<article>#<output>`. */
  bron?: string;
  /** The claim of the login that fills it, when not the field's own. */
  inlog?: string;
}

/** The form of a submission under one article. */
export interface Indieningsvorm {
  soort: string;
  basis: string;
  velden: Veld[];
  /** The article whose decision it asks for. */
  besluit: string;
  /** The cell that decides on it. */
  cel: string;
  prefill?: Record<string, string>;
  systeem?: Record<string, Systeemvulling>;
  verborgen?: string[];
  /** Why the server could not say what it will fill, if so. */
  systeem_fout?: string;
}

/** The form of a decision under one article: what the authority finds. */
export interface Besluitvorm {
  soort: string;
  basis: string;
  cel: string;
  ambtshalve: boolean;
  bevindingen: Veld[];
  uitvoer: string[];
}

/** A fact a cell records, with its fields. */
export interface Handeling {
  feit: string;
  velden: Veld[];
  /**
   * Per field the UI spec has the system fill: the kind of fill (`datum`, the
   * day by the clock, or `waarde`, a fixed value) and the value it records.
   */
  systeem?: Record<string, { soort: 'datum' | 'waarde'; waarde: unknown }>;
}

/** One act after a grant, as the case names it (`na_toekenning`). */
export interface Vervolghandeling {
  /** The fact, `<regelwerk>#<article>#<fact>`. */
  feit: string;
  /** What the button that records it says. */
  knop: string;
  /** What a citizen reads once it is done; `{datum}` stands for its day. */
  stand: string;
}

/** What follows a grant in the case: the acts in their order. */
export interface NaToekenning {
  /** What a citizen reads of a grant before the first act. */
  stand: string;
  handelingen: Vervolghandeling[];
}

/** One grant and how far its acts are (`vervolg::Stand`). */
export interface Vervolgstand {
  /** The decision on the application, the root of the aggregate. */
  besluit: string;
  /** The record that granted, which every act refers to. */
  toekenning: string;
  /** Per act, in order, the day it was done, or null. */
  stappen: { feit: string; gedaan: string | null }[];
  /** The fact of the next act; null once every act is done. */
  volgende: string | null;
  /** For an employee: the day of the record that granted. */
  toegekend_op?: string | null;
  /** For an employee: whom the grant concerns, by the field the login fills. */
  aanvrager?: unknown;
}

/** The facts one cell records. */
export interface CelHandelingen {
  cel: string;
  cel_naam: string;
  handelingen: Handeling[];
}

/** `GET /api/formulier`: the forms of the process today. */
export interface Vormen {
  indieningen: Record<string, Indieningsvorm>;
  besluiten: Record<string, Besluitvorm>;
  eigen: CelHandelingen;
  andere_cellen: CelHandelingen[];
  /** Per field how a screen names it, from the UI spec. */
  teksten?: Record<string, Veldtekst>;
  /** The fields in the order the UI spec names them. */
  volgorde?: string[];
}

/** A submission the case knows, with the cell that decides on it. */
export interface CasusIndiening {
  artikel: string;
  besluit: string;
  cel: string;
  cel_naam: string;
  soort: string;
}

/** `GET /api/casus`. */
export interface Casus {
  id: string;
  titel?: string;
  cel: string;
  cel_naam: string;
  /** The article of the case's own application. */
  artikel: string;
  /** The article of the decision on it. */
  besluit: string;
  /** The output that says whether the decision grants what was asked. */
  uitkomst?: string;
  /** Who decides, as the law names the authority. */
  rol?: string;
  andere_cellen: { cel: string; cel_naam: string }[];
  indieningen: CasusIndiening[];
  /** What follows a grant, where the case says so. */
  na_toekenning?: NaToekenning | null;
  vandaag: string;
}

/**
 * What a record is in the process, as the kern of the chronolexografie poc
 * reads it from the record and the law (`stand::Rol`).
 */
export type Rol = 'indiening' | 'besluit' | 'vastlegging';

/** A decision another record took in the place of a revoked one: no record of its own. */
export interface BesluitInDePlaats {
  basis?: string;
  fields?: Record<string, unknown>;
  outputs?: Record<string, unknown>;
  /** The day of the record that took it. */
  recorded_on: string;
}

/** What answers a submission now (`stand::Antwoord`). */
export type Stukantwoord =
  | { soort: 'open' }
  | { soort: 'besloten'; besluit: string }
  | { soort: 'in_de_plaats'; besluit: string; door: string; uitvoer: string; nieuw: BesluitInDePlaats }
  /** Revoked with nothing in its place; with `na` it waits for a decision after that record. */
  | { soort: 'vervallen'; besluit: string; door: string; na?: string };

/**
 * What the kern says of a list of records (`stand::Stand`): what each is,
 * the last record that revoked a revoked one, and per submission what answers
 * it now. The server sends it beside a chronicle and a dossier.
 */
export interface Stand {
  rollen: Record<string, Rol>;
  herroepen: Record<string, string>;
  antwoorden: Record<string, Stukantwoord>;
}

/** A record of a chronicle (a chronolexogram). */
export interface Gram {
  id: string;
  kind: string;
  basis: string;
  refers_to?: string | null;
  /**
   * What came in from another cell: for a submission that contests a
   * decision of another cell (an appeal to the court), that decision.
   */
  received?: { from: string; id: string } | null;
  fact?: string | null;
  fields?: Record<string, unknown>;
  findings?: Record<string, unknown>;
  outputs?: Record<string, unknown>;
  missing?: string[];
  /** For a decision taken once more: the record it was taken after. */
  after?: string | null;
  recorded_on: string;
  recording_actor: string;
}

/** `GET /api/klok`. */
export interface Klok {
  datum: string;
  modus: string;
  bestuurbaar: boolean;
}

/** What an employee may do for the executor (`GET /api/ik`). */
export interface CelRecht {
  cel: string;
  cel_naam: string;
  lezen: boolean;
  behandelen: boolean;
  beslissen: boolean;
}

/** `GET /api/ik`: who is logged in and what they may do. */
export interface Ik {
  sub: string;
  naam: string | null;
  burger: boolean;
  /** A citizen's identifier, from the claim of their account. */
  identiteit: string | null;
  niveau: string | null;
  /** What the account may do for the executor: the one cell an employee works for, or none. */
  cellen: CelRecht[];
  /** Whether the account enters in the admin panel what an external party did. */
  partijen_invoeren: boolean;
  klok_zetten: boolean;
}

/** Who filled an open term of a decision (`OpenTermFilled` of the engine). */
export interface Ingevuld {
  law_id: string;
  article: string;
  term: string;
  filled_by: unknown;
}

/** An answer of a cell an open term of the decision took. */
export interface Antwoord {
  begrip: string;
  waarde: unknown;
  door: string;
  antwoord: string;
}

/** Why a decision came out so (`kern::Waarom`). */
export interface Waarom {
  ingevuld: Ingevuld[];
  antwoorden: Antwoord[];
  bevindingen: string[];
}

/** What a decision would come to (`kern::Voorvertoning`). */
export interface Voorvertoning {
  voorlopig?: { besluit: Gram; waarom?: Waarom };
  geweigerd?: string;
  stil: Veld[];
}
