/**
 * Load the demo corpus from the static assets copy-demo-corpus.mjs produced:
 * the law catalogue, every law's YAML (text and parsed), the bindings sidecar,
 * the persona profiles, the organisation registry and the demo configuration.
 *
 * Everything is fetched once and kept in a module-level promise, so any view
 * that needs the corpus awaits the same load.
 */
import * as yaml from 'js-yaml';
import type { Produces } from '@regelrecht/frontend-shared/produces';
import { DEFAULT_LOCALE, LOCALES } from '../i18n/index.js';
import { caseRefsFor } from './caseRefs';
import type { CaseRef } from './caseRefs';
import type { OutcomePhrasing } from './outcomePhrasing';
import type { IncomeComponent } from '../simulation/income';
import type { Binding } from './materialize.js';

export type { Binding };

// ---------------------------------------------------------------------------
// The law format, as far as the demo reads it.
//
// The canonical contract is `schema/latest/schema.json`; these types describe
// the parts the demo uses and keep the format's own keys. Nothing validates a
// parsed YAML against them: they say what the code expects, and every object
// keeps an index signature for the fields not listed here.
// ---------------------------------------------------------------------------

/** Where an input gets its value: another regulation's output, or a data source. */
export interface InputSource {
  regulation?: string;
  output?: string;
  parameters?: Record<string, unknown> | null;
  [key: string]: unknown;
}

/** A parameter, input or output entry of `machine_readable.execution`. */
export interface ExecutionField {
  name: string;
  type?: string;
  required?: boolean;
  type_spec?: { unit?: string | null; [key: string]: unknown } | null;
  source?: InputSource | null;
  [key: string]: unknown;
}

/** An action: the operation tree that computes one output. */
export interface ExecutionAction {
  output?: string;
  value?: unknown;
  [key: string]: unknown;
}

/** An article's `machine_readable.execution` section. */
export interface ExecutionSection {
  produces?: Produces;
  parameters?: ExecutionField[] | null;
  input?: ExecutionField[] | null;
  output?: ExecutionField[] | null;
  actions?: ExecutionAction[] | null;
  [key: string]: unknown;
}

/** An article's `machine_readable` section. */
export interface MachineReadableSection {
  definitions?: Record<string, unknown> | null;
  execution?: ExecutionSection | null;
  [key: string]: unknown;
}

/** One article of a regelwerk. */
export interface LawArticle {
  /** A string in most files; unquoted YAML parses it as a number. */
  number: string | number;
  text?: string;
  machine_readable?: MachineReadableSection | null;
  [key: string]: unknown;
}

/** A parsed regelwerk: one version of a regulation. */
export interface LawDocument {
  $id?: string;
  name?: string;
  articles?: LawArticle[];
  [key: string]: unknown;
}

// ---------------------------------------------------------------------------
// The static assets copy-demo-corpus.mjs writes.
// ---------------------------------------------------------------------------

/** One law version in `index.json`, as copy-demo-corpus.mjs lists it. */
export interface LawIndexEntry {
  id: string;
  name: string;
  /** The executing organisation; copy-demo-corpus.mjs refuses a law without one. */
  service: string;
  regulatory_layer?: string;
  valid_from: string;
  uuid?: string | null;
  /** URL of the YAML file under `/data/laws/`. */
  path: string;
  /** The POC address: the law's directory path, without the file name. */
  law_path: string;
  outputs: string[];
  inputs: string[];
}

/** One `.feature` file in `index.json`. */
export interface ScenarioEntry {
  path: string;
  law_path: string;
  title: string;
}

/** `index.json`. */
export interface CorpusIndex {
  laws: LawIndexEntry[];
  scenarios: ScenarioEntry[];
}

export type { CaseRef, OutcomePhrasing };

/** A loaded law version: the index entry plus its YAML, raw and parsed. */
export interface LawEntry extends LawIndexEntry {
  /** raw YAML */
  text: string;
  /** parsed YAML */
  doc: LawDocument;
  /** Set by `loadCorpus` once every law is loaded. */
  caseRefs?: CaseRef[];
}

/** `bindings.yaml`: law id -> input name -> where the demo finds the value. */
export type BindingsDocument = Record<string, Record<string, Binding>>;

/**
 * Register tables of one organisation: table name -> rows. A row is a record
 * keyed by the register's own (Dutch) column names.
 */
export type RegisterTables = Record<string, Record<string, unknown>[]>;

/** A persona in `profiles.yaml`, keyed by BSN. */
export interface PersonaProfile {
  name: string;
  description?: string;
  properties?: string[];
  /** organisation -> its register tables for this person */
  sources?: Record<string, RegisterTables>;
}

/** A case the demo seeds its case store with (`case_seeds` in profiles.yaml). */
export interface CaseSeed {
  service: string;
  law: string;
  table: string;
  bsn_lookup?: { table: string; match_field: string; bsn_field: string };
  /** law parameter -> column of the seed table */
  parameters?: Record<string, string>;
  result?: Record<string, unknown>;
}

/** `profiles.yaml`. */
export interface ProfilesDocument {
  /** Register tables every persona shares: organisation -> tables. */
  globalServices?: Record<string, RegisterTables>;
  case_seeds?: CaseSeed[];
  /** BSN -> persona */
  profiles: Record<string, PersonaProfile>;
}

/** An organisation in `services.yaml`. */
export interface ServiceEntry {
  name?: string;
  /** file name under public/logos/, without `.png` */
  logo?: string;
}

/** `services.yaml`. */
export interface ServicesDocument {
  services?: Record<string, ServiceEntry>;
  /** law id -> executing service */
  laws?: Record<string, string>;
}

/** A law addressed the POC way. */
export interface LawAddress {
  law_path: string;
  service: string;
}

/** A persona as `demo-config.yaml` configures it (`profiles.<key>`). */
export interface DemoProfileConfig {
  name: string;
  /** `burger` or `ondernemer` */
  type: string;
  bsn: string;
  kvk: string | null;
  /** KvK number of the business the profile starts acting for. */
  start_namens?: string;
  default_law?: LawAddress;
  default_feature?: string;
  graph_laws?: string[];
  graph_focus?: string;
  zaaksysteem_service?: string;
  portal_tab_label?: string;
  portal_heading?: string;
  portal_subtitle?: string;
  feature_flags?: Record<string, boolean>;
  sidebar_laws?: string[] | null;
  /** law id -> input name -> declared value */
  declarations?: Record<string, Record<string, unknown>>;
  /** service -> law ids */
  disabled_laws?: Record<string, string[]>;
}

/** One slide of the presentation (`slides`). */
export interface SlideConfig {
  kind: string;
  overline?: string;
  title?: string;
  subtitle?: string;
  footer?: string;
  lead?: string;
  lines?: string[];
  bullets?: string[];
  route?: string;
  highlight?: string;
  profile?: string;
  decks?: string[];
  link?: { href: string; label: string };
}

/** `demo-config.yaml`, or a translated copy of it. */
export interface DemoConfig {
  default_profile?: string;
  profiles?: Record<string, DemoProfileConfig>;
  /** service -> law ids */
  review_laws?: Record<string, string[]>;
  /** service -> law ids */
  hidden_laws?: Record<string, string[]>;
  /** `<service>/<law>` -> output name */
  dashboard_outputs?: Record<string, string>;
  /** `<service>/<law>` -> output names */
  tile_details?: Record<string, string[]>;
  /** `<service>/<law>` -> phrasing */
  outcome_phrasing?: Record<string, OutcomePhrasing>;
  simulation?: { disposable_income?: IncomeComponent[] };
  /** law id -> YAML paths */
  expanded_paths?: Record<string, string[]>;
  /** law id -> YAML paths */
  folded_paths?: Record<string, string[]>;
  slides?: SlideConfig[];
}

/** What `loadCorpus()` resolves to. */
export interface Corpus {
  laws: LawEntry[];
  /** Latest version per law id. */
  latestById: Map<string, LawEntry>;
  lawById: (id: string) => LawEntry | null;
  /** Resolve the POC-style (law_path, service) address to the latest law entry. */
  lawByPath: (lawPath: string, service?: string | null) => LawEntry | null;
  scenarios: ScenarioEntry[];
  bindings: BindingsDocument;
  profiles: ProfilesDocument;
  services: Record<string, ServiceEntry>;
  config: DemoConfig;
  /** locale code -> config; a locale without an overlay is absent. */
  configByLocale: Record<string, DemoConfig>;
  /** locale code -> profiles; a locale without an overlay is absent. */
  profilesByLocale: Record<string, ProfilesDocument>;
}

let corpusPromise: Promise<Corpus> | null = null;

async function fetchText(url: string): Promise<string> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Kon ${url} niet laden (${res.status})`);
  return res.text();
}

async function fetchYaml(url: string): Promise<unknown> {
  return yaml.load(await fetchText(url));
}

export function loadCorpus(): Promise<Corpus> {
  if (corpusPromise) return corpusPromise;
  corpusPromise = (async (): Promise<Corpus> => {
    const index = (await (await fetch('/data/index.json')).json()) as CorpusIndex;
    // De vertaalde talen, in de volgorde van de talentabel. Het Nederlands zit
    // er niet bij: dat is de bron en staat in `demo-config.yaml` zelf.
    const translated = LOCALES.filter((l) => l.code !== DEFAULT_LOCALE);
    // Elke taal van de configuratie wordt opgehaald, niet één op basis van de
    // taal die op dat moment aanstaat: een taalwissel halverwege een
    // presentatie mag geen laadmoment opleveren, en het corpus wordt maar één
    // keer geladen. Het verschil is per taal een bestand van twintig kilobyte.
    const [bindingsDoc, profilesDoc, servicesDoc, configDoc, ...overlays] = await Promise.all([
      fetchYaml('/data/bindings.yaml'),
      fetchYaml('/data/profiles.yaml'),
      fetchYaml('/data/services.yaml'),
      fetchYaml('/data/demo-config.yaml'),
      // `null` bij een ontbrekende overlay: een taal zonder vertaalde
      // configuratie of persona's valt terug op het Nederlands, en dat mag het
      // opstarten niet breken. Config en profiles per taal, in die volgorde,
      // zodat de index hieronder klopt.
      ...translated.flatMap((l) => [
        fetchYaml(`/data/demo-config.${l.code}.yaml`).catch(() => null),
        fetchYaml(`/data/profiles.${l.code}.yaml`).catch(() => null),
      ]),
    ]);
    // The parsed YAML is trusted to have the documented shape; nothing
    // validates it, as before.
    const bindings = bindingsDoc as BindingsDocument;
    const profiles = profilesDoc as ProfilesDocument;
    const services = servicesDoc as ServicesDocument;
    const config = configDoc as DemoConfig;
    const laws: LawEntry[] = await Promise.all(
      index.laws.map(async (entry) => {
        const text = await fetchText(entry.path);
        return { ...entry, text, doc: yaml.load(text) as LawDocument };
      }),
    );
    // Latest version per law id, for the UI (the engine keeps every version and
    // picks by calculation date itself).
    const latestById = new Map<string, LawEntry>();
    for (const law of laws) {
      const cur = latestById.get(law.id);
      if (!cur || law.valid_from > cur.valid_from) latestById.set(law.id, law);
    }
    // Wat een wet uit de besloten zaken van een andere wet leest, zodat de
    // graaf die samenhang toont (data/caseRefs.ts, graph/lawGraph.ts).
    const latestByPath = (lawPath: string, service?: string | null) =>
      [...latestById.values()].find((l) => l.law_path === lawPath && (!service || l.service === service)) ?? null;
    for (const law of laws) law.caseRefs = caseRefsFor(law, bindings, latestByPath);
    return {
      laws,
      latestById,
      lawById: (id: string) => latestById.get(id) ?? null,
      /** Resolve the POC-style (law_path, service) address to the latest law entry. */
      lawByPath: (lawPath: string, service?: string | null) => {
        const candidates = laws.filter(
          (l) => l.law_path === lawPath && (!service || l.service === service),
        );
        candidates.sort((a, b) => b.valid_from.localeCompare(a.valid_from));
        return candidates[0] ?? null;
      },
      scenarios: index.scenarios,
      bindings,
      profiles,
      services: services.services ?? {},
      config,
      /**
       * Dezelfde configuratie per taal, met de vertaalde teksten erin.
       *
       * Alle talen staan naast elkaar in het corpus; welke een scherm ziet,
       * bepaalt de store (`useDemo().corpus`), want dat is de plek die
       * reactief is. Een taal zonder overlay staat er niet in en valt daar
       * terug op het Nederlands, wat beter is dan lege dia's.
       */
      configByLocale: {
        [DEFAULT_LOCALE]: config,
        ...Object.fromEntries(
          translated
            .map((l, i): [string, DemoConfig | null] => [l.code, overlays[i * 2] as DemoConfig | null])
            .filter((e): e is [string, DemoConfig] => !!e[1]),
        ),
      },
      /**
       * Dezelfde persona's met hun vertaalde beschrijving, per taal.
       *
       * Alleen `description` verschilt: namen blijven namen, en de
       * geregistreerde gegevens eronder zijn de invoer van de wet en horen in
       * geen enkele taal vertaald te worden. Een taal zonder overlay staat er
       * niet in en valt daar terug op het Nederlands.
       */
      profilesByLocale: {
        [DEFAULT_LOCALE]: profiles,
        ...Object.fromEntries(
          translated
            .map((l, i): [string, ProfilesDocument | null] => [l.code, overlays[i * 2 + 1] as ProfilesDocument | null])
            .filter((e): e is [string, ProfilesDocument] => !!e[1]),
        ),
      },
    };
  })();
  return corpusPromise;
}

/** Organisation display data for a service code. */
export interface ServiceInfo {
  code: string;
  name: string;
  logo: string | null;
}

/** Organisation display data for a service code. */
export function serviceInfo(corpus: Pick<Corpus, 'services'>, code: string): ServiceInfo {
  const entry: ServiceEntry = corpus.services[code] ?? {};
  return {
    code,
    name: entry.name ?? code,
    logo: entry.logo ? `/logos/${entry.logo}.png` : null,
  };
}
