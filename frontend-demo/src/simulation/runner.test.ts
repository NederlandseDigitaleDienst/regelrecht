/**
 * Welke wetten de simulatie draait.
 *
 * Dezelfde afleiding als het portaal (RFC-038), want een simulatie over een
 * synthetische bevolking hoort precies te doen wat het portaal voor één
 * persoon doet. Dat liep eerder uit elkaar: de simulatie las nog het oude
 * `discoverable`, dat uit het corpus verdwenen is, en draaide daardoor stil
 * op nul wetten.
 */
import { describe, expect, it } from 'vitest';
import { caseSourceLaws, notAppliedFor, simulationLaws, supportingLaws } from './runner';
import type { SimulationLaw } from './runner';
import type { LawDocument } from '../data/loadCorpus';

const law = (id: string, legalCharacter?: string, params = ['bsn'], outputs = ['bedrag']): SimulationLaw => ({
  id,
  name: id,
  // A fixture with only the fields the selection reads (no article numbers).
  doc: {
    articles: [
      {
        machine_readable: {
          execution: {
            ...(legalCharacter ? { produces: { legal_character: legalCharacter } } : {}),
            parameters: params.map((name) => ({ name })),
            output: outputs.map((name) => ({ name })),
          },
        },
      },
    ],
  } as LawDocument,
});

const corpusOf = (...laws: SimulationLaw[]) => ({ latestById: new Map(laws.map((l) => [l.id, l])) });

describe('simulationLaws', () => {
  it('draait de burgerwetten die een beschikking opleveren', () => {
    const corpus = corpusOf(
      law('zorgtoeslag', 'BESCHIKKING'),
      law('brp', 'INFORMATIEF'),
      law('alcoholwet', 'BESCHIKKING', ['kvk_nummer']),
    );
    const { runnable } = simulationLaws(corpus, 'burgers');
    expect(runnable.map((l) => l.id)).toEqual(['zorgtoeslag']);
  });

  it('draait de ondernemerswetten voor ondernemers', () => {
    const corpus = corpusOf(
      law('zorgtoeslag', 'BESCHIKKING'),
      law('alcoholwet', 'BESCHIKKING', ['kvk_nummer']),
    );
    const { runnable } = simulationLaws(corpus, 'ondernemers');
    expect(runnable.map((l) => l.id)).toEqual(['alcoholwet']);
  });

  it('neemt een rechtspositie mee', () => {
    // Kiesgerechtigdheid: geen besluit, wel iets wat de burger aangaat.
    const corpus = corpusOf(law('kieswet', 'RECHTSPOSITIE'));
    expect(simulationLaws(corpus, 'burgers').runnable.map((l) => l.id)).toEqual(['kieswet']);
  });

  it('laat delegatieleveranciers buiten de simulatie', () => {
    const corpus = corpusOf(
      law('gezag', 'RECHTSPOSITIE', ['bsn'], ['heeft_delegaties', 'subject_ids', 'delegation_types']),
    );
    expect(simulationLaws(corpus, 'burgers').runnable).toEqual([]);
  });

  it('volgt de zichtbaarheidskeuze van de demo', () => {
    const corpus = corpusOf(law('zorgtoeslag', 'BESCHIKKING'), law('bibob', 'BESCHIKKING'));
    const { runnable } = simulationLaws(corpus, 'burgers', (l) => l.id !== 'bibob');
    expect(runnable.map((l) => l.id)).toEqual(['zorgtoeslag']);
  });

  it('zet een wet die de bevolking niet kan voeden apart', () => {
    const corpus = corpusOf(law('vergunning', 'BESCHIKKING', ['bsn', 'lievelingskleur']));
    const { runnable, skipped } = simulationLaws(corpus, 'burgers');
    expect(runnable).toEqual([]);
    expect(skipped).toEqual([expect.objectContaining({ missing: ['lievelingskleur'] })]);
  });
});

describe('caseSourceLaws', () => {
  it('names the laws other laws read as decided cases, so the simulation applies for them first', () => {
    const bindings = {
      precario: {
        vergunde_oppervlakte: { kind: 'cases', select_on: [{ name: 'law', value: 'apv/terrassen' }, { name: 'kvk_nummer', value: '$kvk_nummer' }] },
        heeft_terras: { kind: 'table', table: 'vestigingen', select_on: [] },
      },
      awb: { zaak: { kind: 'cases' } },
    };
    expect([...caseSourceLaws(bindings)]).toEqual(['apv/terrassen']);
  });

  it('laat een wet weg die alleen haar eigen zaken leest', () => {
    const bindings = {
      'apv/ontheffingspas': {
        verleende_ontheffingen_dit_jaar: { kind: 'cases', select_on: [{ name: 'law', value: 'apv/ontheffingspas' }, { name: 'kvk_nummer', value: '$kvk_nummer' }] },
      },
    };
    expect([...caseSourceLaws(bindings)]).toEqual([]);
  });
});

describe('notAppliedFor', () => {
  const terras = { law_path: 'apv/terrassen' };
  const precario = { law_path: 'precario' };
  const paths = new Set(['apv/terrassen']);

  it('laat een aanvraagwet weg voor wie hem niet aanvroeg', () => {
    expect(notAppliedFor(terras, { aanvragen: [] }, paths)).toBe(true);
    expect(notAppliedFor(terras, {}, paths)).toBe(true);
  });

  it('rekent een aanvraagwet door voor wie hem aanvroeg', () => {
    expect(notAppliedFor(terras, { aanvragen: ['apv/terrassen'] }, paths)).toBe(false);
  });

  it('rekent een wet die geen aanvraag kent voor iedereen door', () => {
    expect(notAppliedFor(precario, { aanvragen: [] }, paths)).toBe(false);
  });
});

describe('supportingLaws', () => {
  // Een wet met constanten, en een wet waar een andere wet naar verwijst.
  const doc = ({ defs, reads = [], implementsLaw }: { defs?: Record<string, number>; reads?: string[]; implementsLaw?: string } = {}): LawDocument => ({
    articles: [
      {
        machine_readable: {
          ...(implementsLaw ? { implements: [{ law: implementsLaw, article: '4', open_term: 'standaardpremie' }] } : {}),
          ...(defs ? { definitions: defs } : {}),
          execution: { input: reads.map((regulation) => ({ name: regulation, source: { regulation } })) },
        },
      },
    ],
  }) as LawDocument; // a fixture without article numbers
  const entry = (id: string, d: LawDocument): SimulationLaw => ({ id, name: id, doc: d });
  const constants = (d: LawDocument) => Object.values(d?.articles?.[0]?.machine_readable?.definitions ?? {});

  it('vindt de regeling die een open term van de wet invult', () => {
    // De standaardpremie: geen constante van de zorgtoeslagwet, wel van de
    // ministeriële regeling die hem elk jaar vaststelt.
    const zorgtoeslag = entry('zorgtoeslagwet', doc({ defs: { drempel: 1 } }));
    const regeling = entry('standaardpremie', doc({ defs: { standaardpremie_2025: 211200 }, implementsLaw: 'zorgtoeslagwet' }));
    const corpus = corpusOf(zorgtoeslag, regeling);
    expect(supportingLaws(corpus, [zorgtoeslag], constants).map((l) => l.id)).toEqual(['standaardpremie']);
  });

  it('volgt verwijzingen door, en laat wetten zonder constanten weg', () => {
    const top = entry('top', doc({ reads: ['midden'] }));
    const midden = entry('midden', doc({ reads: ['onder'] }));
    const onder = entry('onder', doc({ defs: { grens: 5 } }));
    const corpus = corpusOf(top, midden, onder);
    expect(supportingLaws(corpus, [top], constants).map((l) => l.id)).toEqual(['onder']);
  });

  it('volgt de zichtbaarheidskeuze van de demo, zoals simulationLaws', () => {
    const top = entry('top', doc({ reads: ['verborgen', 'zichtbaar'] }));
    const verborgen = entry('verborgen', doc({ defs: { x: 1 } }));
    const zichtbaar = entry('zichtbaar', doc({ defs: { y: 2 } }));
    const corpus = corpusOf(top, verborgen, zichtbaar);
    expect(supportingLaws(corpus, [top], constants, (l) => l.id !== 'verborgen').map((l) => l.id)).toEqual(['zichtbaar']);
  });

  it('biedt een regeling die een open term invult ook aan als de demo hem verbergt', () => {
    // hidden_laws houdt de standaardpremie-regeling van het portaal; hier
    // hoort hij juist, anders valt de eerste "wat als" van de presentator weg.
    const zorgtoeslag = entry('zorgtoeslagwet', doc({ reads: ['hulpwet'] }));
    const regeling = entry('standaardpremie', doc({ defs: { standaardpremie_2025: 211200 }, implementsLaw: 'zorgtoeslagwet' }));
    const hulpwet = entry('hulpwet', doc({ defs: { x: 1 } }));
    const corpus = corpusOf(zorgtoeslag, regeling, hulpwet);
    const visible = (l: SimulationLaw) => l.id === 'zorgtoeslagwet';
    expect(supportingLaws(corpus, [zorgtoeslag], constants, visible).map((l) => l.id)).toEqual(['standaardpremie']);
  });

  it('noemt een gesimuleerde wet niet nog eens, en overleeft een kring', () => {
    const a = entry('a', doc({ defs: { x: 1 }, reads: ['b'] }));
    const b = entry('b', doc({ defs: { y: 2 }, reads: ['a'] }));
    const corpus = corpusOf(a, b);
    expect(supportingLaws(corpus, [a], constants).map((l) => l.id)).toEqual(['b']);
  });
});
