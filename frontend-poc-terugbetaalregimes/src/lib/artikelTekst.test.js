// De wettekst van een artikel bijwerken, met behoud van de blokstijl.
//
// Waarom dit een eigen test verdient: een artikel draagt zijn wettekst als
// blok-scalar, en het corpus gebruikt daar twee vormen voor (`>-` en `|`). Die
// betekenen echt iets anders: `|` houdt regeleinden vast (opsommingen,
// leden), `>-` vouwt ze samen tot een lopende zin. Wie dat door elkaar haalt,
// verandert de wet zonder dat er een woord verandert.
import { describe, it, expect } from 'vitest';
import * as yaml from 'js-yaml';
import { patchArticleText, readArticleText } from './yamlPatch.js';

const GEVOUWEN = `articles:
  - number: '6.10'
    text: >-
      De draagkracht wordt vastgesteld op vier procent van het
      toetsingsinkomen boven de draagkrachtvrije voet.
    url: https://example.invalid/6.10
    machine_readable:
      definitions:
        voet:
          value: 1
`;

const LETTERLIJK = `articles:
  - number: '34'
    text: |
      1. De school ontvangt bekostiging.
      2. De drempel is vier leerlingen.
    machine_readable:
      definitions:
        drempel:
          value: 4
`;

describe('patchArticleText', () => {
  it('vervangt de tekst en houdt de gevouwen stijl aan', () => {
    const nieuw = patchArticleText(GEVOUWEN, '6.10', 'De draagkracht wordt vastgesteld op vijf procent.');

    expect(nieuw).toContain('text: >-');
    expect(readArticleText(nieuw, '6.10')).toBe('De draagkracht wordt vastgesteld op vijf procent.');
  });

  // `|` houdt regeleinden vast; die omzetten naar `>-` zou de leden van een
  // artikel aan elkaar plakken tot één zin.
  it('houdt de letterlijke stijl aan, met regeleinden', () => {
    const nieuw = patchArticleText(LETTERLIJK, '34', '1. De school ontvangt bekostiging.\n2. De drempel vervalt.');

    expect(nieuw).toContain('text: |');
    expect(readArticleText(nieuw, '34')).toBe('1. De school ontvangt bekostiging.\n2. De drempel vervalt.\n');
  });

  it('laat de rest van het artikel met rust', () => {
    const nieuw = patchArticleText(GEVOUWEN, '6.10', 'Kort.');

    expect(nieuw).toContain('url: https://example.invalid/6.10');
    expect(yaml.load(nieuw).articles[0].machine_readable.definitions.voet.value).toBe(1);
  });

  it('laat andere artikelen ongemoeid', () => {
    const twee = `articles:
  - number: '1'
    text: >-
      Eerste artikel.
  - number: '2'
    text: >-
      Tweede artikel.
`;
    const nieuw = patchArticleText(twee, '2', 'Gewijzigd tweede artikel.');

    expect(readArticleText(nieuw, '1')).toBe('Eerste artikel.');
    expect(readArticleText(nieuw, '2')).toBe('Gewijzigd tweede artikel.');
  });

  it('schrijft meerregelige tekst met de juiste inspringing', () => {
    const nieuw = patchArticleText(LETTERLIJK, '34', 'Eerste regel.\nTweede regel.\nDerde regel.');

    expect(() => yaml.load(nieuw)).not.toThrow();
    expect(readArticleText(nieuw, '34')).toBe('Eerste regel.\nTweede regel.\nDerde regel.\n');
  });

  it('meldt het als het artikel of de tekst niet bestaat', () => {
    expect(() => patchArticleText(GEVOUWEN, '9.9', 'x')).toThrow(/artikel 9\.9 niet gevonden/);
    const zonderTekst = "articles:\n  - number: '1'\n    url: https://example.invalid/1\n";
    expect(() => patchArticleText(zonderTekst, '1', 'x')).toThrow(/geen wettekst/);
  });

  // Een dubbele punt of een streepje aan het begin breekt een losse
  // YAML-waarde; in een blok-scalar hoort dat gewoon te mogen.
  it('verdraagt leestekens die YAML anders in de war sturen', () => {
    const lastig = 'Onze Minister stelt vast: het percentage, bedoeld in lid 1, is nul.';
    const nieuw = patchArticleText(GEVOUWEN, '6.10', lastig);

    expect(() => yaml.load(nieuw)).not.toThrow();
    expect(readArticleText(nieuw, '6.10')).toBe(lastig);
  });
});
