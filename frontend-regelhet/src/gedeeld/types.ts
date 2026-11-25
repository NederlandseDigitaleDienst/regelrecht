// The shapes the server in packages/regelhet answers with. Their keys are
// those of the kern of the chronolexografie poc (`velden`, `bevindingen`,
// `op`), as the API passes them on; only what the screens read is typed.
import type { Stap } from '@regelrecht/frontend-shared/chronolex/keten';
import type { Systeemvulling } from '@regelrecht/frontend-shared/chronolex/voorinvulling';

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
  /** A yes and a no in words on a form, such as "Onderzoek nodig" for no. */
  when_true?: string;
  when_false?: string;
  /** The texts on a screen that shows a recorded submission, where they differ from the form's. */
  record?: { label?: string; help?: string; group?: string; summary?: boolean; when_true?: string; when_false?: string };
}

/** How the system fills a field of the application (`fill: {by: system}`): shared with the poc. */
export type { Systeemsoort, Systeemvulling } from '@regelrecht/frontend-shared/chronolex/voorinvulling';

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
  /**
   * Per field the UI spec prefills (`fill: {from}`), what the form starts
   * with for this applicant: the value, the source, and why it gives none.
   * The applicant may change it.
   */
  voorgevuld?: Record<string, { waarde: unknown; bron?: string; fout?: string }>;
  systeem?: Record<string, Systeemvulling>;
  verborgen?: string[];
  /** Why the server could not say what it will fill, if so. */
  systeem_fout?: string;
  /**
   * The field that holds the last day to make the submission in time, as the
   * article that names its moment reads it (Awb 6:9); a submission against a
   * decision takes it from the decision. Absent when the law names none.
   */
  termijn?: string | null;
  /** Per output of its article, what the law says of it. */
  uitleg?: Record<string, Uitvoeruitleg>;
}

/** What the law says of an output: its description, type and unit (`kern::uitleg`). */
export interface Uitvoeruitleg {
  beschrijving?: string | null;
  type?: string | null;
  eenheid?: string | null;
}

/** The form of a decision under one article: what the authority finds. */
export interface Besluitvorm {
  soort: string;
  basis: string;
  cel: string;
  ambtshalve: boolean;
  bevindingen: Veld[];
  uitvoer: string[];
  /** Per output of the deciding article, what the law says of it. */
  uitleg?: Record<string, Uitvoeruitleg>;
  /** The findings the UI spec has the system fill, with the value it would record today. */
  systeem?: Record<string, { soort: 'datum' | 'waarde'; waarde: unknown }>;
  /** The findings the UI spec hides (`visibility: hidden`). */
  verborgen?: string[];
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
  /** What the button that records it says, for an act a person records. */
  knop?: string;
  /** Set when the system records the act itself, by the article it names. */
  door_het_systeem?: { artikel: string };
  /** What a citizen reads once it is done; `{datum}` stands for its day. */
  stand: string;
}

/** An application the case offers about a grant once its acts are done, such as a duplicate. */
export interface Vervolgaanvraag {
  /** The article that establishes it. */
  artikel: string;
  /** What the way in to it says. */
  knop: string;
  /** The title of its form. */
  titel: string;
  /** What the form says before the fields. */
  uitleg: string;
  /** The output of the decision on it that says whether it grants. */
  uitkomst: string;
}

/** What follows a grant in the case: the acts in their order. */
export interface NaToekenning {
  /** What a citizen reads of a grant before the first act. */
  stand: string;
  handelingen: Vervolghandeling[];
  /** What a citizen reads of a grant a new one took the place of. */
  vervallen?: string;
  aanvragen?: Vervolgaanvraag[];
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
  /** For an employee: the applicant's name, as the application recorded it from the login. */
  naam?: string | null;
}

/**
 * A request of the policy in a citizen's dossier, by the ids of its records
 * there: the request, the other party's receipt, the answer and the
 * executor's receipt of it. An id is null when that record is not (yet) in
 * the dossier.
 */
export interface VerzoekInDossier {
  verzoek: string;
  ontvangen: string | null;
  antwoord: string | null;
  antwoord_ontvangen: string | null;
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
  /** Per field the screen asks in parts, those parts in order. */
  delen?: Record<string, Velddeel[]>;
  /** The fields the form does not submit without, where the law lets them be empty. */
  verplicht?: string[];
  /** Which message to the citizen follows from which record, with the text layer (`berichten.rs`). */
  berichten?: Berichtenspec;
  /** The tables a screen shows beside a submission, from the UI spec (`table`). */
  tabellen?: Tabel[];
  /** Every element of the UI spec, its layers merged: who sees it, its fields and its parts. */
  elementen?: Uielement[];
}

/** An element of the UI spec as a screen lists it per role (`ui_spec::Overzicht`). */
export interface Uielement {
  id: string;
  element?: string;
  rol?: string;
  bouwt_op?: string;
  rol_generiek?: string;
  velden: { naam: string; zichtbaarheid?: string; wijzigbaar?: boolean; vulling?: string; te_bouwen?: boolean }[];
  delen?: { naam: string; plek?: string; velden: string[]; feiten: string[] }[];
}

/** A column of a table of the UI spec: the key in a row and its heading. */
export interface Tabelkolom {
  name: string;
  label?: string;
}

/**
 * A table element of the UI spec (`ui_spec::Tabel`): its rows come from an
 * array the submission holds (`veld`), an array its article gives
 * (`uitkomst`), or every record of a fact (`feit`); `herkomst` names the
 * facts of other cells a row comes from.
 */
export interface Tabel {
  naam: string;
  element?: string;
  rol?: string;
  rol_generiek?: string;
  veld?: string;
  uitkomst?: string;
  feit?: string;
  kolommen: Tabelkolom[];
  herkomst?: { fact: string; match: string[] }[];
  label?: string;
  help?: string;
}

/** One record a row of a table comes from: which cell, what it records, which record, on which day. */
export interface Tabelbron {
  cel: string;
  wat: string;
  stuk: string;
  op: string;
}

/** One row of a table beside a submission, with where it comes from. */
export interface Tabelrij {
  waarden: Record<string, unknown>;
  herkomst: Tabelbron[];
  /** The facts of `herkomst` that no record matched. */
  zonder_herkomst?: string[];
}

/** `GET /api/backoffice/tabellen/{id}`: the tables beside one submission. */
export interface Tabellen {
  tabellen: { tabel: Tabel; rijen: Tabelrij[]; leeg?: string }[];
  /** Per cell a source names, its name. */
  cel_namen: Record<string, string>;
}

/** A place in a message, in the order a message has them (`berichten::PLEKKEN`). */
export type Berichtplek =
  | 'kop'
  | 'inleiding'
  | 'dictum'
  | 'grondslag'
  | 'motivering'
  | 'onderbouwing'
  | 'specificatie'
  | 'mededeling'
  | 'ondertekening'
  | 'rechtsmiddelen'
  | 'bijlage';

/**
 * Where a key takes its value from (`berichten::BRONSOORTEN`), with at most
 * one test that makes it a condition, and the form its value is written in.
 */
export interface Sleutelbron {
  uitkomst?: string;
  veld?: string;
  keten?: string;
  indiening?: string;
  record?: 'recorded_on' | 'id' | 'kenmerk';
  label?: string;
  afzender?: string;
  zin?: string;
  sleutel?: string;
  is?: unknown;
  gelijk?: unknown;
  groter_dan?: number;
  gevuld?: boolean;
  niet_gevuld?: boolean;
  alle?: Sleutelbron[];
  een_van?: Sleutelbron[];
  niet?: Sleutelbron;
  vorm?: 'bedrag' | 'datum' | 'getal' | 'tekst';
  /** A field read from the record alone, not from the records it answers. */
  eigen?: boolean;
}

/** One part of a kind of message: its place, what requires it, where its content comes from. */
export interface Berichtdeel {
  id: string;
  plek: Berichtplek;
  vereist_door?: string;
  bron?: {
    tekst?: (string | number)[];
    regels?: string[];
    tabel?: { rijen: string[]; totaal?: string };
    bijlage?: Bijlagebron;
    wet?: 'grondslag' | 'keten' | 'rechtsmiddelen' | 'ondertekening';
  };
  als?: string;
  nldd?: string;
  titel?: string;
}

/** A column of an annex: its heading and its cell, as label keys of the text layer (`bijlagen`). */
export interface Bijlagekolom {
  /** The label of the heading. */
  kop: string;
  /** The label whose text, with the fields of the row in braces, is the cell. */
  tekst: string;
  /** A label for a second line under the cell, shown when all its fields have a value. */
  onder?: string;
  /** Show the second line only when this field of the row is greater than this number. */
  onder_als?: { veld: string; groter_dan: number };
}

/**
 * An annex in columns, such as a calculation per body (`bron.bijlage`): the rows of a key whose value is a list
 * of rows, grouped by a field, with the amount of each row summed per group and in total. Every text comes from
 * the text layer (`bijlagen.<labels>`); the spec names only fields and label keys.
 */
export interface Bijlagebron {
  labels: string;
  /** A key whose value is a list of rows, or rows the spec builds from keys: per row field a key, under `als`. */
  rijen: string | { als?: string; velden: Record<string, string> }[];
  groep?: string;
  /** The field of a row that is its amount; without it the annex has no sums. */
  bedrag?: string;
  /** The fields of a row that are amounts in eurocent. */
  bedragen?: string[];
  kolommen: Bijlagekolom[];
  /** A key whose value the total must be, such as the outcome that is the sum by the model. */
  totaal?: string;
}

/** What must hold between the parts of a message. */
export interface Berichtinvariant {
  soort: 'bedrag_is_totaal' | 'oordeel_past';
  bedrag?: string;
  specificatie?: string;
  oordelen?: Record<string, Sleutelbron>;
}

/** A kind of message: the record it follows from, its pattern, keys, parts and invariants. */
export interface Berichtsoort {
  id: string;
  naam?: string;
  patroon: string;
  vastlegging: {
    rol: Rol;
    artikel: string;
    feit?: string;
    rechtskarakter?: string;
    uitkomst?: string;
    stap?: string;
    /** When two kinds follow from records of one article: the condition on the record for this one. */
    als?: Sleutelbron;
  };
  sjabloon?: string;
  sleutels?: Record<string, Sleutelbron>;
  delen: Berichtdeel[];
  invarianten?: Berichtinvariant[];
  status?: { bekendgemaakt: Sleutelbron };
  kanaal?: { vast?: Kanaalcode; sleutel?: string; waarden?: Record<string, Kanaalcode> };
}

export type Kanaalcode = 'email' | 'portaal' | 'papier';

/** One paragraph of a template of the text layer. */
export interface Alinea {
  kop?: string;
  tekst?: string;
  als?: string;
  punten?: string[];
}

/** A template of the text layer: the texts of one message. */
export interface Sjabloon {
  id: string;
  titel?: string;
  betreft?: string;
  betreft_bij?: Record<string, string>;
  aanhef?: string;
  alineas?: Alinea[];
  slot?: string;
  ondertekenaar?: string;
}

/** The text layer of the authority: who signs, and the templates. */
export interface Tekstlaag {
  afzender?: { organisatie?: string; namens?: string; in_mandaat?: boolean; rollen?: Record<string, string> };
  sjablonen?: Sjabloon[];
  zinnen?: Record<string, string>;
  /** Per annex the texts of its headings and cells (`Bijlagebron.labels`), with the fields of a row in braces. */
  bijlagen?: Record<string, Record<string, string>>;
  /**
   * The head and foot of a letter of the sender: the text in the band on top (`lint`, the name of the sender
   * when absent; never a logo), the return address over the address block, and the lines of the footer.
   */
  briefhoofd?: { lint?: string; retouradres?: string; voettekst?: string[] };
}

/**
 * How a message goes out (`verzending`): the fact by which the sender records the message sent, with its content
 * frozen (`feit`), the fact by which it forwards a sent message to another cell (`doorsturen`), and per further
 * parameter of the fact where its value comes from (`velden`), such as whom it is about.
 */
export interface Verzending {
  feit: string;
  doorsturen?: string;
  velden?: Record<string, Sleutelbron>;
}

/** The message spec as the forms carry it. */
export interface Berichtenspec {
  verzending?: Verzending;
  berichtsoorten: Berichtsoort[];
  tekstlaag?: Tekstlaag;
  meldingen?: string[];
}

/** One input of a field the screen asks in parts; the form joins them into the one value the law asks. */
export interface Velddeel {
  naam: string;
  label?: string;
  help?: string;
  /** The browser's autofill hint, such as `postal-code`. */
  autocomplete?: string;
  /** The on-screen keyboard a phone raises, such as `numeric`. */
  keyboard?: string;
  /** How wide the input is, as a CSS length, where it should be as wide as what goes in it. */
  width?: string;
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
  /** The regelwerken the cell runs: `$id`, name, and directory in the corpus. */
  regelwerken?: { id: string; naam?: string | null; map: string }[];
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
  /** Records facts, sends copies of decisions and takes decisions. */
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
  /** The external party this server works for, when it is not the executor's own: its staff enter there what it does. */
  partij?: { cel: string; cel_naam: string } | null;
  klok_zetten: boolean;
  /** Whether the account may ask Vlam for a draft explanation of a decision: a decider, with Vlam configured. */
  concepttoelichting?: boolean;
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
  /** The trace of the engine condensed into steps (`kern::Stap`). */
  keten?: Stap[];
}

/** What a decision would come to (`kern::Voorvertoning`). */
export interface Voorvertoning {
  voorlopig?: { besluit: Gram; waarom?: Waarom };
  geweigerd?: string;
  stil: Veld[];
  /** Every finding a policy may leave to the authority, whether or not this case reaches it. */
  mogelijk?: Veld[];
  /** What the system would then record itself: per act its fact, the article and the record it refers to. */
  automatisch?: { feit: string; artikel: string; verwijst_naar?: string | null }[];
}

/** Who is to act next (`timeline::Actor` of the server). */
export type AanZet = 'uitvoerder' | 'systeem' | 'externe_partij' | 'indiener';

/** One thing that can or must happen next about an application (`timeline::Next`). */
export type VolgendeStap =
  /** The cell `cel` decides on the submission `op`; with `na` again, after that record. */
  | { soort: 'besluit'; wie: AanZet; cel: string; op: string; na?: string }
  /** The cell `cel` answers the request `op` with the fact `feit`; `taak` names that as a step to take. */
  | { soort: 'antwoord'; wie: AanZet; cel: string; op: string; feit: string; taak?: string }
  /** The act `feit` after the grant `op`; with `artikel`, the system takes it by that article; `taak` names it as a step to take. */
  | { soort: 'handeling'; wie: AanZet; cel: string; op: string; feit: string; artikel?: string; taak?: string }
  /** The cell `cel` sends a copy of its decision `op`. */
  | { soort: 'toezending'; wie: AanZet; cel: string; op: string }
  /** A submission against the decision `op` can be made until `tot`, the value of its output `veld`. */
  | { soort: 'termijn'; wie: AanZet; op: string; veld: string; tot: string };

/**
 * The timeline of one application (`GET /api/dossier/tijdlijn/{id}`,
 * `GET /api/backoffice/tijdlijn/{id}`): its records in the order they
 * happened, what the kern says of them, and what comes next. The server
 * keeps it to what the reader may see.
 */
export interface Tijdlijn {
  /** The application at the top. */
  stuk: string;
  grammen: Gram[];
  stand: Stand;
  /** Per record the system made by an article of the policy, that article. */
  door_het_systeem: Record<string, string>;
  volgende: VolgendeStap[];
}

/** The chronicle of one cell. */
export interface Kroniek {
  cel: string;
  cel_naam: string;
  kop: number;
  grammen: Gram[];
  /** What the kern says of its records: what each is, which are revoked, what answers each submission. */
  stand: Stand;
  /** Per record the system made by an article of the policy, that article. */
  door_het_systeem?: Record<string, string>;
}

/** A submission that waits for a decision of the executor. */
export interface Wachtend {
  stuk: Gram;
  /** The record to decide after, when its last decision was revoked: decide again. */
  na?: string;
}

/** `GET /api/backoffice/werkvoorraad`: what waits for a decision of the executor. */
export interface Werkvoorraad {
  cel: string;
  cel_naam: string;
  open: Wachtend[];
}

/**
 * A request the executor's policy made when an application came in, as the
 * executor sees it: its own record, the party it went to, and the record of
 * the answer that came back to it. Never the party's own records.
 */
export interface EigenVerzoek {
  verzoek: Gram;
  aan: string;
  aan_naam: string;
  open: boolean;
  antwoord_ontvangen: Gram | null;
}
