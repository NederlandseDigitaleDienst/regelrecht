import { afterEach, describe, expect, it } from 'vitest';
import { GRAMMAR } from '@regelrecht/frontend-shared/gherkin';
import { FEATURE_KEYWORDS_NL, TRANSLATED_STEP_IDS, featureKeywords, matchStep, renderStep, renderStepNl, scenarioTitle } from './gherkinNl';
import { adoptLocale } from '../i18n/index.js';

describe('renderStepNl', () => {
  it('translates the keyword and phrases a matched step in Dutch', () => {
    const step = { keyword: 'Given', text: 'the calculation date is "2025-01-01"' };
    expect(renderStepNl(step)).toEqual({ keyword: 'Gegeven', text: 'de peildatum is "2025-01-01"', matched: true });
  });

  it('renders the law-scoped data source step with source, law and key', () => {
    const step = { keyword: 'And', text: 'the following "RvIG" data with key "bsn" for law "wet_brp":' };
    const out = renderStepNl(step);
    expect(out.keyword).toBe('En');
    expect(out.text).toBe('de volgende gegevens van "RvIG" voor "wet_brp", gesleuteld op "bsn":');
  });

  it('renders assertions', () => {
    expect(renderStepNl({ keyword: 'Then', text: 'output "hoogte_toeslag" equals 165412' }).text).toBe(
      'is "hoogte_toeslag" gelijk aan 165412',
    );
    expect(renderStepNl({ keyword: 'And', text: 'output "is_verzekerde" is true' }).text).toBe('is "is_verzekerde" waar');
  });

  it('falls back to the English text for a step outside the grammar', () => {
    const out = renderStepNl({ keyword: 'When', text: 'something the grammar does not know' });
    expect(out).toEqual({ keyword: 'Als', text: 'something the grammar does not know', matched: false });
  });

  it('has a Dutch line for every core step of the grammar', () => {
    // A new core step without a template would show up in English on stage.
    for (const entry of GRAMMAR.filter((e) => e.tier === 'core')) {
      const english = entry.template(entry.argTypes.map((t) => (t === 'number' ? '1' : 'x')));
      expect(matchStep(english)?.entry.id).toBe(entry.id);
      expect(TRANSLATED_STEP_IDS.has(entry.id), entry.id).toBe(true);
      expect(renderStepNl({ keyword: 'Given', text: english }).matched).toBe(true);
    }
    expect(FEATURE_KEYWORDS_NL.Feature).toBe('Functionaliteit');
  });
});

describe('in het Engels', () => {
  afterEach(() => adoptLocale('nl'));

  it('toont een stap in zijn canonieke vorm, onvertaald', () => {
    // De canonieke grammatica is Engels, dus "vertalen naar het Engels" is
    // hier: niets doen. Dat is waarom deze stap goedkoop was.
    adoptLocale('en');
    const step = { keyword: 'Given', text: 'the calculation date is "2025-01-01"' };
    expect(renderStep(step)).toEqual({ keyword: 'Given', text: 'the calculation date is "2025-01-01"', matched: true });
  });

  it('blijft melden of een stap in de grammatica staat', () => {
    // Het scherm markeert een onbekende stap in beide talen; dat oordeel mag
    // niet verdwijnen omdat er niet vertaald wordt.
    adoptLocale('en');
    expect(renderStep({ keyword: 'Given', text: 'iets wat de grammatica niet kent' }).matched).toBe(false);
  });

  it('gebruikt de Engelse Gherkin-sleutelwoorden', () => {
    expect(featureKeywords().Feature).toBe('Functionaliteit');
    adoptLocale('en');
    expect(featureKeywords().Feature).toBe('Feature');
    expect(featureKeywords().Background).toBe('Background');
  });

  it('vertaalt een titel en laat een onbekende staan', () => {
    adoptLocale('en');
    expect(scenarioTitle('Berekening Zorgtoeslag 2024')).toBe('Calculation of zorgtoeslag (healthcare allowance) 2024');
    expect(scenarioTitle('Een titel die niet bestaat')).toBe('Een titel die niet bestaat');
    adoptLocale('nl');
    expect(scenarioTitle('Berekening Zorgtoeslag 2024')).toBe('Berekening Zorgtoeslag 2024');
  });
});

describe('een taal zonder eigen sjablonen', () => {
  afterEach(() => adoptLocale('nl'));

  // Fries is de eerste taal die dit raakt. Hij staat in de talentabel, maar de
  // stappen zijn geschreven zinnen en die schrijft een vertaler, geen tabel.
  // Tot die er zijn hoort er iets wáárs op het scherm te staan.

  it('toont de stap zoals hij in het bestand staat', () => {
    adoptLocale('fy');
    const step = { keyword: 'Given', text: 'the calculation date is "2025-01-01"' };
    expect(renderStep(step)).toEqual({ keyword: 'Given', text: 'the calculation date is "2025-01-01"', matched: true });
  });

  it('houdt de Engelse sleutelwoorden en niet de Nederlandse', () => {
    // De stap eronder staat in het Engels, dus `Functionaliteit` erboven zou
    // een taal suggereren die er niet is. Dit is het verschil tussen een
    // terugval op de bron en een terugval op de canonieke vorm.
    adoptLocale('fy');
    expect(featureKeywords().Feature).toBe('Feature');
    expect(featureKeywords().Background).toBe('Background');
  });

  it('blijft melden of een stap in de grammatica staat', () => {
    adoptLocale('fy');
    expect(renderStep({ keyword: 'Given', text: 'iets wat de grammatica niet kent' }).matched).toBe(false);
  });
});
