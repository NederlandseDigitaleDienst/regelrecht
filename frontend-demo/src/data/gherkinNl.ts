/**
 * How a BDD step is shown to the audience.
 *
 * The scenario files stay in the canonical vocabulary from bdd/grammar.yaml so
 * the Rust runner and the browser runner read the same text; this module only
 * decides how a step is *shown*. Every template is keyed by grammar step id, so
 * a new step in the grammar shows up in its canonical form until it gets a
 * line here.
 *
 * That canonical form is English, which is why the English side of this module
 * is empty: showing a step in English means not translating it at all. The
 * Dutch templates are the work; English is what the file already says.
 *
 * Dit is de enige plek in de app waar een taal toevoegen méér is dan een regel
 * in de talentabel. Een sjabloon is een Nederlandse zin die iemand geschreven
 * heeft, geen waarde die uit een tabel te halen valt. Een taal zonder sjablonen
 * krijgt daarom de canonieke Engelse stap te zien: dat is wat er in het bestand
 * staat, het is waar, en het is beter dan een half vertaalde zin. Het Engels
 * heeft om diezelfde reden geen ingang.
 */
import { GRAMMAR, matchStep } from '@regelrecht/frontend-shared/gherkin';
import type { GrammarEntry, Step } from '@regelrecht/frontend-shared/gherkin';
import { DEFAULT_LOCALE, currentLocale } from '../i18n/index.js';
import generatedTitles from '../i18n/scenarioTitles.generated.js';

/** locale -> Dutch title -> translated title. Indexed by a runtime locale code, so a locale may be absent. */
const titles: Record<string, Record<string, string | undefined> | undefined> = generatedTitles;

const ENTRIES: readonly GrammarEntry[] = GRAMMAR;

/** How a matched step reads in a language, from its captured arguments. */
type StepTemplate = (args: string[]) => string;

/** The keywords a `.feature` file uses for its blocks. */
export interface FeatureKeywords {
  Feature: string;
  Background: string;
  Scenario: string;
}

/** A step as the audience sees it. */
export interface RenderedStep {
  keyword: string;
  text: string;
  /** Whether the grammar knows the step. */
  matched: boolean;
}

// Re-exported, not redefined: matching a step against the canonical grammar is
// the shared runner's job, and this module only decides how a matched step is
// shown. ScenariosView and the tests here keep importing it from this module.
export { matchStep };

/**
 * De Gherkin-sleutelwoorden per taal.
 *
 * Een taal die hier niet in staat houdt de Engelse sleutelwoorden, want dat is
 * wat er in het `.feature`-bestand staat.
 */
const KEYWORDS: Record<string, Record<string, string> | undefined> = {
  nl: {
    Given: 'Gegeven',
    When: 'Als',
    Then: 'Dan',
    And: 'En',
    But: 'Maar',
    '*': '*',
  },
};

const TEMPLATES: { nl: Record<string, StepTemplate>; [code: string]: Record<string, StepTemplate> | undefined } = {
  nl: {
    set_calculation_date: (a) => `de peildatum is ${q(a[0])}`,
    load_law: (a) => `het regelwerk ${q(a[0])} is geladen`,
    set_parameter_string: (a) => `parameter ${q(a[0])} is ${q(a[1])}`,
    set_parameter_number: (a) => `parameter ${q(a[0])} is ${a[1]}`,
    set_parameters_table: () => 'de volgende parameters:',
    set_data_source: (a) => `de volgende gegevens uit ${q(a[0])}, gesleuteld op ${q(a[1])}:`,
    set_data_source_for_law: (a) => `de volgende gegevens van ${q(a[0])} voor ${q(a[2])}, gesleuteld op ${q(a[1])}:`,
    set_parameter_collection: (a) => `parameter ${q(a[0])} is de verzameling:`,
    evaluate: (a) => `${q(a[1])} wordt uitgevoerd voor ${q(a[0])}`,
    evaluate_outputs: (a) => `${q(a[1])} wordt uitgevoerd voor ${q(a[0])}`,
    assert_succeeds: () => 'slaagt de uitvoering',
    assert_fails: () => 'mislukt de uitvoering',
    assert_fails_with: (a) => `mislukt de uitvoering met ${q(a[0])}`,
    assert_boolean_true: (a) => `is ${q(a[0])} waar`,
    assert_boolean_false: (a) => `is ${q(a[0])} onwaar`,
    assert_equals_number: (a) => `is ${q(a[0])} gelijk aan ${a[1]}`,
    assert_equals_string: (a) => `is ${q(a[0])} gelijk aan ${q(a[1])}`,
    assert_null: (a) => `is ${q(a[0])} afwezig`,
    assert_unknown: (a) => `is ${q(a[0])} onbekend`,
    assert_unknown_for: (a) => `is ${q(a[0])} onbekend bij gebrek aan ${q(a[1])}`,
    assert_contains: (a) => `bevat ${q(a[0])} ${q(a[1])}`,
    assert_exact_outputs: (a) => `bevat de uitkomst precies ${q(a[0])}`,
  },
};

/** The canonical step keywords, as a `.feature` file writes them. */
const CANONICAL_KEYWORDS = ['Given', 'When', 'Then', 'And', 'But'];

/**
 * Every step keyword the active language shows. A language without its own
 * keywords shows the canonical English ones, as `renderStep` does.
 */
export function stepKeywords(): string[] {
  const own = KEYWORDS[currentLocale()];
  return own ? Object.values(own) : CANONICAL_KEYWORDS;
}

/** Grammar step ids that have a Dutch rendering (for the coverage test). */
export const TRANSLATED_STEP_IDS = new Set(Object.keys(TEMPLATES.nl));

function q(s: string | undefined): string {
  return `"${s}"`;
}

/**
 * A step as the audience sees it, in the language that is on.
 *
 * In English the step is printed as it stands in the file: that *is* the
 * canonical grammar, so there is nothing to render. `matched` still says
 * whether the step is one the grammar knows, because the view marks an
 * unknown step either way.
 */
export function renderStep(step: Pick<Step, 'keyword' | 'text'>): RenderedStep {
  const match = matchStep(step.text);
  // Geen sjablonen voor deze taal: toon de stap zoals hij in het bestand staat.
  // Dat is de canonieke Engelse vorm, en die is juist.
  const templates = TEMPLATES[currentLocale()];
  if (!templates) return { keyword: step.keyword, text: step.text, matched: !!match };
  const keyword = KEYWORDS[currentLocale()]?.[step.keyword] ?? step.keyword;
  const template = match ? templates[match.entry.id] : null;
  // `template` is only set when `match` is.
  return { keyword, text: template && match ? template(match.args) : step.text, matched: !!match };
}

/** Kept for callers that still import the old name. */
export const renderStepNl = renderStep;

/**
 * Every step as a pair of templates: how `from` shows it and how `to` does,
 * with `{0}`, `{1}` where the arguments go. A language without templates shows
 * the canonical step. The recorded walkthrough uses these to find a step it
 * clicked in one language on a screen in another (walkthrough/translateText.ts).
 */
export function stepTemplatePairs(from: string, to: string): [string, string][] {
  const render = (code: string, entry: GrammarEntry, args: string[]): string => (TEMPLATES[code]?.[entry.id] ?? entry.template)(args);
  return ENTRIES.map((entry): [string, string] => {
    const args = entry.argTypes.map((_, i) => `{${i}}`);
    return [render(from, entry, args), render(to, entry, args)];
  }).filter(([a, b]) => a !== b);
}

const FEATURE_KEYWORDS: { nl: FeatureKeywords; en: FeatureKeywords; [code: string]: FeatureKeywords | undefined } = {
  nl: { Feature: 'Functionaliteit', Background: 'Achtergrond', Scenario: 'Scenario' },
  en: { Feature: 'Feature', Background: 'Background', Scenario: 'Scenario' },
};

/**
 * The Gherkin keywords, in the language that is on.
 *
 * Een taal zonder eigen sleutelwoorden valt terug op het Engels en niet op het
 * Nederlands: de rest van het scherm toont dan ook de canonieke Engelse stappen,
 * en `Functionaliteit` boven een Engelse stap zou een taal suggereren die er
 * niet is.
 */
export function featureKeywords(): FeatureKeywords {
  return FEATURE_KEYWORDS[currentLocale()] ?? FEATURE_KEYWORDS.en;
}

export const FEATURE_KEYWORDS_NL = FEATURE_KEYWORDS.nl;
/**
 * De titel van een feature of scenario, in de taal die aanstaat.
 *
 * De `.feature`-bestanden blijven Nederlands: de Rust-runner en de
 * browser-runner lezen dezelfde bestanden, en `just bdd-demo` toetst erop. De
 * vertaling staat er dus naast, gesleuteld op de Nederlandse titel zelf.
 *
 * Een titel die er niet in staat blijft Nederlands. Dat is bij een scenario
 * minder erg dan het klinkt: de stappen eronder zijn canoniek Engels en dragen
 * de inhoud.
 */
export function scenarioTitle(title: string | null | undefined): string {
  if (!title || currentLocale() === DEFAULT_LOCALE) return title ?? '';
  return titles[currentLocale()]?.[title] ?? title;
}
