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

/** How a screen names a field, where the UI spec says so (`label`, `help`). */
export interface Veldtekst {
  label?: string;
  help?: string;
}

/** How the system fills a field of the application (`fill: {by: system}`). */
export interface Systeemvulling {
  /** A value the system knows itself. */
  waarde?: unknown;
  /** A source the cell computes. */
  bron?: string;
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
  vandaag: string;
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
