import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import * as yaml from 'js-yaml';
import { regimeVanCohort, cohortUitleg, COHORTGRENS } from '../src/lib/regimeFacts.js';
import { scenarioRecord } from '../src/lib/scenarioRecord.js';
import { dataDir } from './helpers/casusPaden.js';

const PERSONAS = yaml.load(
  readFileSync(resolve(dataDir, 'personas.yaml'), 'utf-8'),
).personas;

/**
 * Het regime kies je niet: de wet leidt het af uit je cohort. Deze test legt
 * de grenzen vast zoals ze als definitie in de WSF 2000 staan, want een
 * verschuiving daarvan verandert stilzwijgend wie onder welke regels valt.
 */
describe('cohortroutering', () => {
  const record = (jaar, onderwijssoort = 'hbo', lllk = false) => ({
    eerste_studiefinanciering: `${jaar}-09-01`,
    onderwijssoort,
    is_levenlanglerenkrediet: lllk,
  });

  it('vóór 2009 gelden de oudste regels', () => {
    expect(regimeVanCohort(record(2008))).toBe('SF15_OUD');
    expect(regimeVanCohort(record(COHORTGRENS.sf15_oud - 1))).toBe('SF15_OUD');
  });

  it('ho splitst in 2015, mbo pas in 2023', () => {
    expect(regimeVanCohort(record(2014, 'hbo'))).toBe('SF15_NIEUW');
    expect(regimeVanCohort(record(2015, 'hbo'))).toBe('SF35');
    expect(regimeVanCohort(record(2015, 'wo'))).toBe('SF35');
    // Hetzelfde jaar, ander regime: dat is precies de cohortval.
    expect(regimeVanCohort(record(2015, 'mbo'))).toBe('SF15_NIEUW');
    expect(regimeVanCohort(record(2023, 'mbo'))).toBe('SF35');
  });

  it('een levenlanglerenkrediet heeft eigen regels, ongeacht het jaar', () => {
    expect(regimeVanCohort(record(2005, 'hbo', true))).toBe('SF15_LLLK');
    expect(regimeVanCohort(record(2024, 'mbo', true))).toBe('SF15_LLLK');
  });

  it('legt uit waarom, met het jaartal erbij', () => {
    expect(cohortUitleg(record(2005))).toContain('2005');
    expect(cohortUitleg(record(2005))).toContain('oudste');
    expect(cohortUitleg(record(2020, 'mbo'))).toContain('mbo');
    expect(cohortUitleg(record(2005, 'hbo', true))).toContain('levenlanglerenkrediet');
  });

  it('komt overeen met de regimes van de scenario\'s in personas.yaml', () => {
    // De YAML noemt het regime in de omschrijving; de afleiding moet daarmee
    // kloppen, anders staat er iets anders op het scherm dan in de tekst.
    const verwacht = {
      Priya: 'SF15_OUD', Wouter: 'SF15_OUD', Kwame: 'SF15_OUD', Mariska: 'SF15_OUD',
      Aisha: 'SF15_NIEUW', Yusuf: 'SF35', Els: 'SF15_LLLK',
    };
    for (const p of PERSONAS) {
      expect(regimeVanCohort(p), p.naam).toBe(verwacht[p.naam]);
    }
  });
});

/**
 * Een zelf toegevoegd scenario moet exact het schema van personas.yaml
 * hebben, anders struikelt de simulatie erover of ontbreekt er stilletjes een
 * veld in de doorrekening.
 */
describe('scenario-record', () => {
  const VERPLICHT = [
    'bsn', 'naam', 'geboortejaar', 'huishoudtype', 'heeft_partner', 'partnerinkomen',
    'inkomen', 'inkomensgroei', 'schuld', 'eerste_studiefinanciering', 'onderwijssoort',
    'is_levenlanglerenkrediet', 'keuzemomenten',
  ];

  it('de scenario\'s uit de YAML hebben alle velden', () => {
    for (const p of PERSONAS) {
      for (const veld of VERPLICHT) expect(p, `${p.naam}.${veld}`).toHaveProperty(veld);
    }
  });

  // Het formulier van ScenarioFormulier, in hele euro's en procenten.
  const vorm = {
    naam: 'Test', schuld: 20000, inkomen: 32000, partnerinkomen: 15000,
    huishoudtype: 'alleenstaand', startjaar: 2016, onderwijssoort: 'hbo',
    lllk: false, geboortejaar: 1995, groei: 2,
  };

  it('een samengesteld record heeft dezelfde velden', () => {
    const record = scenarioRecord(vorm, 'eigen-1');

    // Niet meer en niet minder dan een persona uit de YAML, op de markering
    // `eigen` na.
    const { eigen, ...rest } = record;
    expect(eigen).toBe(true);
    expect(Object.keys(rest).sort()).toEqual(Object.keys(PERSONAS[0]).sort());
    for (const veld of VERPLICHT) expect(record).toHaveProperty(veld);
    // Bedragen in eurocent, net als in de YAML.
    expect(record.schuld).toBe(2000000);
    expect(record.inkomen).toBe(3200000);
    // 2 procent groei wordt 0,02.
    expect(record.inkomensgroei).toBeCloseTo(0.02, 6);
    expect(regimeVanCohort(record)).toBe('SF35');
  });

  it('rekent partnerinkomen alleen mee bij een partner', () => {
    const zonder = scenarioRecord(vorm, 'eigen-1');
    expect(zonder.heeft_partner).toBe(false);
    expect(zonder.partnerinkomen).toBe(0);

    const met = scenarioRecord({ ...vorm, huishoudtype: 'paar_met_kind' }, 'eigen-1');
    expect(met.heeft_partner).toBe(true);
    expect(met.partnerinkomen).toBe(1500000);
  });
});
