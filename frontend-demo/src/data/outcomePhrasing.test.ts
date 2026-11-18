import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import * as yaml from 'js-yaml';
import { dateInputFor, phraseOutcome, phrasingFor } from './outcomePhrasing';

/**
 * A tile that says "Je voldoet aan de voorwaarden" over a bare number tells the
 * citizen nothing they did not already have to work out. The phrasing turns it
 * into a sentence addressed to them ("Je huurtoeslag is waarschijnlijk € 302,96
 * per jaar"). Everything below guards the ways that sentence can come out wrong
 * on screen: a placeholder that leaks, a lead without a number, a law whose
 * wording nobody wrote yet.
 */

const root = path.resolve(import.meta.dirname, '../../..');
// The real demo-config.yaml, read as the config type `phrasingFor` takes; the
// assertions below check its contents.
const config = yaml.load(fs.readFileSync(path.join(root, 'corpus/demo/demo-config.yaml'), 'utf8')) as Parameters<typeof phrasingFor>[0];

/** An amount law: a lead, a unit, and a sentence for when nothing is granted. */
const huurtoeslag = {
  lead: 'Je huurtoeslag is waarschijnlijk',
  unit: 'per jaar',
  none: 'Je krijgt waarschijnlijk geen huurtoeslag.',
};

/** A yes/no law: a verdict instead of an amount, and a date in the lead. */
const kieswet = {
  lead: 'Voor de verkiezingen van {date} heb je',
  lead_no_date: 'Voor de verkiezingen heb je',
  date_input: 'verkiezingsdatum',
  yes: 'STEMRECHT',
  no: 'GEEN stemrecht',
};

describe('phrasingFor', () => {
  it('finds a law by service and path, as the config keys it', () => {
    expect(phrasingFor(config, 'TOESLAGEN', 'wet_op_de_huurtoeslag')?.lead).toBe('Je huurtoeslag is waarschijnlijk');
    // A nested path is part of the key, so two bijstand variants stay distinct.
    expect(phrasingFor(config, 'SZW', 'participatiewet/bijstand')?.unit).toBe('per maand');
  });

  it('returns null for a law nobody wrote a sentence for', () => {
    expect(phrasingFor(config, 'TOESLAGEN', 'wet_die_niet_bestaat')).toBeNull();
    expect(phrasingFor(null, 'TOESLAGEN', 'wet_op_de_huurtoeslag')).toBeNull();
  });
});

describe('an amount law', () => {
  it('wraps the amount in a sentence with its unit', () => {
    expect(phraseOutcome(huurtoeslag, { met: true, value: '€ 302,96' })).toEqual({
      lead: 'Je huurtoeslag is waarschijnlijk',
      headline: '€ 302,96',
      unit: 'per jaar',
    });
  });

  it('states the outcome in words when the conditions do not hold', () => {
    // Not "€ 0,00" and not "Niet van toepassing": the citizen is told what it
    // means for them, which is that they get nothing.
    expect(phraseOutcome(huurtoeslag, { met: false, value: '€ 302,96' })).toEqual({
      lead: '',
      headline: 'Je krijgt waarschijnlijk geen huurtoeslag.',
      unit: null,
    });
  });

  it('does the same when the law granted nothing at all', () => {
    // A met law with no amount would otherwise put a lead over an empty space.
    expect(phraseOutcome(huurtoeslag, { met: true, value: null })?.headline).toBe(
      'Je krijgt waarschijnlijk geen huurtoeslag.',
    );
    expect(phraseOutcome(huurtoeslag, { met: true, value: undefined })?.headline).toBe(
      'Je krijgt waarschijnlijk geen huurtoeslag.',
    );
  });

  it('leaves an unknown amount to the general rendering instead of saying nothing is owed', () => {
    expect(phraseOutcome(huurtoeslag, { met: true, value: 'onbekend', unknown: true })).toBeNull();
  });

  it('falls back rather than showing half a sentence', () => {
    // Wording that is missing the piece it needs must not reach the screen; the
    // tile keeps its general rendering instead.
    expect(phraseOutcome({ lead: 'Je huurtoeslag is waarschijnlijk' }, { met: false, value: null })).toBeNull();
    expect(phraseOutcome({ unit: 'per jaar' }, { met: true, value: '€ 302,96' })).toBeNull();
  });
});

describe('a yes/no law', () => {
  it('states the verdict, in both directions, with no unit after it', () => {
    expect(phraseOutcome(kieswet, { met: true, isYesNo: true, value: 'Ja', date: '29 oktober 2025' })).toEqual({
      lead: 'Voor de verkiezingen van 29 oktober 2025 heb je',
      headline: 'STEMRECHT',
      unit: null,
    });
    expect(phraseOutcome(kieswet, { met: false, isYesNo: true, value: 'Nee', date: '29 oktober 2025' })?.headline).toBe(
      'GEEN stemrecht',
    );
  });

  it('is recognised by its wording even when the value is not a boolean', () => {
    // The primary output of a yes/no law is not always typed boolean; the
    // presence of `yes`/`no` decides, so the amount branch never claims it.
    expect(phraseOutcome(kieswet, { met: true, value: '€ 12,00', date: '29 oktober 2025' })?.headline).toBe('STEMRECHT');
  });

  it('is written that way in the config, not just in this test', () => {
    // The fixture above mirrors demo-config.yaml; bind the two so a change to
    // the real wording cannot leave these assertions passing against a copy.
    expect(phrasingFor(config, 'KIESRAAD', 'kieswet')).toMatchObject(kieswet);
  });

  it('falls back to the lead written for a missing date', () => {
    // The date comes out of the evaluation and can be absent. Dropping only the
    // placeholder would strand the preposition ("Voor de verkiezingen van heb
    // je"), so the whole date clause goes: `lead_no_date` says what is left.
    // Only whoever wrote the sentence knows which words belonged to the date.
    for (const outcome of [{ met: true, isYesNo: true }, { met: true, isYesNo: true, date: null }]) {
      expect(phraseOutcome(kieswet, outcome)?.lead).toBe('Voor de verkiezingen heb je');
    }
  });

  it('drops the placeholder when no lead_no_date is written, leaking no literal {date}', () => {
    // A lead that ends on the date needs no second wording; whatever happens,
    // a literal "{date}" must never reach the screen.
    const noFallback = { lead: 'Je hebt op {date}', 'yes': 'STEMRECHT', 'no': 'GEEN stemrecht' };
    const lead = phraseOutcome(noFallback, { met: true, isYesNo: true })?.lead;
    expect(lead).toBe('Je hebt op');
    expect(lead).not.toContain('{date}');
  });

  it('leaves a lead without a placeholder untouched', () => {
    const kinderbijslag = phrasingFor(config, 'SVB', 'algemene_kinderbijslagwet');
    expect(phraseOutcome(kinderbijslag, { met: true, isYesNo: true })).toEqual({
      lead: 'Je hebt',
      headline: 'RECHT OP KINDERBIJSLAG',
      unit: null,
    });
  });
});

describe('a law with no entry', () => {
  it('returns null so the tile keeps the general rendering', () => {
    expect(phraseOutcome(null, { met: true, value: '€ 302,96' })).toBeNull();
    expect(phraseOutcome(undefined, { met: false, value: null })).toBeNull();
    // The whole outcome argument may be absent too (a tile still computing).
    expect(phraseOutcome(null)).toBeNull();
  });
});

describe('dateInputFor', () => {
  it('names the input that carries the date, and nothing when there is none', () => {
    expect(dateInputFor(kieswet)).toBe('verkiezingsdatum');
    expect(dateInputFor(huurtoeslag)).toBeNull();
    expect(dateInputFor(null)).toBeNull();
  });
});
