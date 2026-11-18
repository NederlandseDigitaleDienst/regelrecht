import { afterEach, describe, expect, it } from 'vitest';
import { fieldSpec, formatDate, formatDateTime, formatMissing, formatValue, humanize, intlLocale, isAmountSpec, numericImpact, setGlossary, untranslated, verdictOf } from './format';
import { adoptLocale } from '../i18n/index.js';
import glossaries from '../i18n/glossary.generated.js';

const UNKNOWN = { __unknown: true, missing: [{ law: 'zorgtoeslagwet', name: 'huurprijs', kind: 'no_data' }, { law: 'wet_inkomstenbelasting', name: 'spaargeld', kind: 'no_data' }] };

const DOC = {
  articles: [
    {
      machine_readable: {
        execution: {
          parameters: [{ name: 'bsn', type: 'string' }],
          input: [{ name: 'leeftijd', type: 'number', type_spec: { unit: 'years' } }],
          output: [{ name: 'hoogte_toeslag', type: 'amount', type_spec: { unit: 'eurocent', precision: 0 } }],
        },
      },
    },
  ],
};

describe('fieldSpec', () => {
  it('finds outputs, inputs and parameters by name', () => {
    expect(fieldSpec(DOC, 'hoogte_toeslag')?.type).toBe('amount');
    expect(fieldSpec(DOC, 'leeftijd')?.type_spec?.unit).toBe('years');
    expect(fieldSpec(DOC, 'bsn')?.type).toBe('string');
    expect(fieldSpec(DOC, 'onbekend')).toBeNull();
    expect(fieldSpec(null, 'x')).toBeNull();
  });
});

describe('formatValue', () => {
  it('renders eurocent amounts as euros', () => {
    expect(formatValue(165412, fieldSpec(DOC, 'hoogte_toeslag'))).toBe('€\u00a01.654,12');
    expect(formatValue(3610, { type: 'number', type_spec: { unit: 'eurocent' } })).toBe('€\u00a036,10');
  });

  it('renders units, booleans, absence and unknowns in Dutch', () => {
    // null is an absence the data states; the engine's Unknown is a fact nobody has.
    expect(formatValue(null)).toBe('geen');
    expect(formatValue(UNKNOWN)).toBe('onbekend');
    expect(formatValue(undefined)).toBe('onbekend');
    expect(formatValue(true)).toBe('Ja');
    expect(formatValue(false)).toBe('Nee');
    expect(formatValue(35, fieldSpec(DOC, 'leeftijd'))).toBe('35 jaar');
    expect(formatValue(12.5, { type_spec: { unit: 'percentage' } })).toBe('12,5%');
  });

  it('renders dates, snake_case strings and collections', () => {
    expect(formatValue('2025-03-01')).toBe('1 maart 2025');
    expect(formatValue('ALLEENSTAANDE_OUDER')).toBe('ALLEENSTAANDE OUDER');
    expect(formatValue([])).toBe('geen');
    expect(formatValue(['a', 'b'])).toBe('a, b');
    expect(formatValue([{ x: 1 }])).toBe('1 item');
    expect(formatValue({ straat: 'Kade', nummer: 1, plaats: null })).toBe('Kade 1');
  });
});

describe('formatMissing / verdictOf', () => {
  it('names its own missing facts, and another law once for all the facts it misses', () => {
    expect(formatMissing(UNKNOWN, { ownLaw: 'zorgtoeslagwet', lawName: (id) => (id === 'wet_inkomstenbelasting' ? 'Wet IB' : id) })).toBe('ontbreekt: huurprijs, gegevens uit Wet IB');
    expect(formatMissing(UNKNOWN)).toBe('ontbreekt: gegevens uit zorgtoeslagwet, gegevens uit wet_inkomstenbelasting');
    const terrace = { __unknown: true, missing: ['obstakelvrije_ruimte', 'terras_oppervlakte', 'beschikbare_oppervlakte'].map((name) => ({ law: 'terrassen', name, kind: 'no_data' })) };
    expect(formatMissing(terrace, { ownLaw: 'precario', lawName: () => 'Terrasvergunning' })).toBe('ontbreekt: gegevens uit Terrasvergunning');
    // The data lineage is about the detail and keeps every fact.
    expect(formatMissing(UNKNOWN, { ownLaw: 'zorgtoeslagwet', lawName: () => 'Wet IB', group: false })).toBe('ontbreekt: huurprijs, spaargeld (Wet IB)');
    expect(formatMissing(null)).toBe('');
    expect(formatMissing(42)).toBe('');
  });

  it('reads the verdict without ever taking an unknown for a yes', () => {
    expect(verdictOf({ voldoet_aan_voorwaarden: true })).toBe(true);
    expect(verdictOf({ voldoet_aan_voorwaarden: false })).toBe(false);
    expect(verdictOf({ voldoet_aan_voorwaarden: UNKNOWN })).toBe('unknown');
    expect(verdictOf({ voldoet_aan_voorwaarden: null })).toBe(false);
    expect(verdictOf({ bedrag: 1 })).toBeNull();
    expect(verdictOf(null)).toBeNull();
  });
});

describe('helpers', () => {
  it('isAmountSpec accepts the amount type and the eurocent unit', () => {
    expect(isAmountSpec({ type: 'amount' })).toBe(true);
    expect(isAmountSpec({ type: 'number', type_spec: { unit: 'eurocent' } })).toBe(true);
    expect(isAmountSpec({ type: 'number' })).toBe(false);
    expect(isAmountSpec(null)).toBe(false);
  });

  it('numericImpact scales eurocents to euros and ignores non-numbers', () => {
    expect(numericImpact(250000, { type: 'amount' })).toBe(2500);
    expect(numericImpact(4, { type: 'number' })).toBe(4);
    expect(numericImpact('x', { type: 'amount' })).toBe(0);
  });

  it('humanize turns an identifier into a label', () => {
    expect(humanize('hoogte_toeslag')).toBe('Hoogte toeslag');
    expect(humanize('')).toBe('');
  });

  it('humanize schrijft een afkorting als afkorting', () => {
    // Zonder dit werd het "Agp vergunning vereist", wat als woord leest.
    expect(humanize('agp_vergunning_vereist')).toBe('AGP vergunning vereist');
    expect(humanize('heeft_haccp_verplichting')).toBe('Heeft HACCP verplichting');
    expect(humanize('kvk_nummer')).toBe('KvK nummer');
    // Een afkorting vooraan houdt haar eigen schrijfwijze.
    expect(humanize('bsn')).toBe('BSN');
    expect(humanize('ww_uitkering_per_maand')).toBe('WW uitkering per maand');
    // Een woord dat toevallig op een afkorting lijkt blijft ongemoeid.
    expect(humanize('wonen_in_nederland')).toBe('Wonen in nederland');
  });

  it('formatDate leaves an unparsable value alone', () => {
    expect(formatDate('geen datum')).toBe('geen datum');
  });
});

describe('in English', () => {
  afterEach(() => {
    adoptLocale('nl');
    setGlossary(null);
  });

  it('formats numbers and money the British way, in euros', () => {
    // en-GB and not en-US: day-first dates and 24-hour time match how a Dutch
    // government screen states them. The currency stays EUR in both; only the
    // separators and the symbol's position move.
    adoptLocale('en');
    expect(intlLocale()).toBe('en-GB');
    expect(formatValue(165412, fieldSpec(DOC, 'hoogte_toeslag'))).toBe('€1,654.12');
    expect(formatValue(12.5, { type_spec: { unit: 'percentage' } })).toBe('12.5%');
    expect(formatDate('2025-03-01')).toBe('1 March 2025');
    expect(formatDateTime('2025-03-01T14:30:00')).toMatch(/^1 Mar[a-z]*,? 14:30$/);
  });

  it('says the two kinds of nothing with two different words', () => {
    adoptLocale('en');
    expect(formatValue(null)).toBe('none');
    expect(formatValue(UNKNOWN)).toBe('unknown');
    expect(formatValue(true)).toBe('Yes');
    expect(formatValue(false)).toBe('No');
    expect(formatValue(35, fieldSpec(DOC, 'leeftijd'))).toBe('35 years');
    expect(formatValue([{ x: 1 }])).toBe('1 item');
    expect(formatValue([{ x: 1 }, { x: 2 }])).toBe('2 items');
  });

  it('names the missing facts in English, with the glossary applied', () => {
    // The labels come from the shipped glossary, so this also proves the
    // generated module reaches `humanize` rather than the empty default.
    adoptLocale('en');
    expect(formatMissing(UNKNOWN, { ownLaw: 'zorgtoeslagwet', lawName: (id) => id })).toBe(
      'missing: rent, details from wet_inkomstenbelasting',
    );
  });

  it('switches back cleanly, so a cached formatter cannot pin the language', () => {
    // An Intl formatter bakes its locale in at construction. Built once at
    // module level, it would keep formatting in Dutch after a switch.
    adoptLocale('en');
    expect(formatValue(1654.12, { type_spec: { unit: 'euro' } })).toBe('€1,654.12');
    adoptLocale('nl');
    expect(formatValue(1654.12, { type_spec: { unit: 'euro' } })).toBe('€\u00a01.654,12');
  });
});

describe('humanize with a glossary', () => {
  afterEach(() => {
    adoptLocale('nl');
    setGlossary(null);
  });

  it('leaves Dutch alone in Dutch, glossary or not', () => {
    setGlossary({ words: { hoogte: 'amount', toeslag: 'allowance' } });
    expect(humanize('hoogte_toeslag')).toBe('Hoogte toeslag');
  });

  it('composes a label word by word', () => {
    adoptLocale('en');
    setGlossary({ words: { hoogte: 'amount of', toeslag: 'allowance' } });
    expect(humanize('hoogte_toeslag')).toBe('Amount of allowance');
  });

  it('prefers a whole-name entry over composing', () => {
    adoptLocale('en');
    setGlossary({
      words: { inkomen: 'income', uit: 'from', arbeid: 'labour' },
      names: { inkomen_uit_arbeid: 'employment income' },
    });
    expect(humanize('inkomen_uit_arbeid')).toBe('Employment income');
  });

  it('prefers a per-law entry over the general one', () => {
    adoptLocale('en');
    setGlossary({ names: { vermogen: 'capacity' }, laws: { participatiewet: { vermogen: 'assets' } } });
    expect(humanize('vermogen', { lawId: 'participatiewet' })).toBe('Assets');
    expect(humanize('vermogen')).toBe('Capacity');
  });

  it('falls back to the whole Dutch name rather than half-translating', () => {
    // "Has partner inkomen" reads like a bug; the Dutch label reads like
    // something not translated yet, which is the truth.
    adoptLocale('en');
    setGlossary({ words: { heeft: 'has', partner: 'partner' } });
    expect(humanize('heeft_partner_inkomen')).toBe('Heeft partner inkomen');
    expect(untranslated.has('heeft_partner_inkomen')).toBe(true);
  });

  it('keeps abbreviations as abbreviations in both languages', () => {
    adoptLocale('en');
    setGlossary({ words: { vereist: 'required', vergunning: 'permit' } });
    expect(humanize('agp_vergunning_vereist')).toBe('AGP permit required');
  });
});

describe('de meegeleverde woordenlijst', () => {
  afterEach(() => adoptLocale('nl'));

  it('vertaalt de namen die op elk scherm staan', () => {
    // Deze vier komen in vrijwel elke tegel voor; gaan die terug naar het
    // Nederlands, dan leest de hele demo half vertaald.
    adoptLocale('en');
    expect(humanize('voldoet_aan_voorwaarden')).toBe('Meets conditions');
    expect(humanize('hoogte_toeslag')).toBe('Allowance amount');
    expect(humanize('heeft_partner')).toBe('Has a partner');
    expect(humanize('toetsingsinkomen')).toBe('Assessment income');
  });

  it('laat afkortingen staan zoals ze geschreven horen', () => {
    adoptLocale('en');
    expect(humanize('heeft_geldige_vog')).toBe('Has valid VOG');
    expect(humanize('is_verzekerde_zorgtoeslag')).toBe('Is insured for healthcare allowance');
  });

  it('levert alleen woorden op, geen booleans of leegte', () => {
    // `null` kaal in YAML is leegte en `true`/`false` zijn booleans, en dan
    // wordt het label "Null" of "False" in plaats van een woord. (`no` en
    // `yes` zouden dat in YAML 1.1 ook zijn; js-yaml volgt 1.2 en leest ze als
    // woorden, maar de lijst quote ze toch, want yamllint vraagt erom.)
    //
    // Over elke taal, niet alleen over het Engels: elke woordenlijst die
    // meegeleverd wordt kan dezelfde fout bevatten.
    for (const [code, g] of Object.entries(glossaries)) {
      const nonStrings = Object.entries(g.words).filter(([, v]) => typeof v !== 'string');
      expect(nonStrings, `woordenlijst ${code}`).toEqual([]);
    }
    adoptLocale('en');
    expect(humanize('geen_recht')).toBe('No right');
  });

  it('zet een Nederlandse woordvolgorde recht waar die niet meekan', () => {
    // Woord voor woord zou dit "Advice degree of danger" en "Meets the
    // article 8" opleveren: elk woord goed, de zin fout.
    adoptLocale('en');
    expect(humanize('advies_mate_van_gevaar')).toBe('Advised degree of danger');
    expect(humanize('voldoet_aan_artikel_8')).toBe('Meets article 8');
  });
});
