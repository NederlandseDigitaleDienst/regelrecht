import { describe, expect, it } from 'vitest';
import {
  decisionOf,
  legalCharacterOf,
  producedDecision,
  producedItems,
  producedSubmission,
  submissionKindOf,
  withDecisionField,
  type ProducedItem,
} from './produces';

const object = { legal_character: 'BESCHIKKING', submission: { kind: 'AANVRAAG' } };
const list = [
  { type: 'AANVRAAG', requests: 'leerplichtwet_1969#13a', parameters: ['bsn_jongere'] },
  { type: 'BESCHIKKING', decision_type: 'TOEKENNING', outputs: ['verlof_verleend'] },
];

describe('produces', () => {
  it('reads the object form as one produced item', () => {
    expect(producedItems(object)).toEqual([object]);
    expect(producedDecision(object)).toBe(object);
    expect(producedSubmission(object)).toBe(object);
    expect(legalCharacterOf(object)).toBe('BESCHIKKING');
    expect(submissionKindOf(object)).toBe('AANVRAAG');
  });

  it('reads the list form item by item, by type', () => {
    expect(producedItems(list)).toBe(list);
    expect(producedDecision(list)).toBe(list[1]);
    expect(producedSubmission(list)).toBe(list[0]);
    expect(legalCharacterOf(list[0])).toBeNull();
    expect(submissionKindOf(list[0])).toBe('AANVRAAG');
    expect(legalCharacterOf(list[1])).toBe('BESCHIKKING');
    expect(submissionKindOf(list[1])).toBeNull();
  });

  it('reads an objection as a submission, and the decision on it as the decision (Awb 7:11)', () => {
    const objection = [
      { type: 'BEZWAAR', requests: 'algemene_wet_bestuursrecht#7:11' },
      { type: 'BESCHIKKING', outputs: ['bezwaar_ontvankelijk', 'bestreden_besluit_herroepen'] },
    ];
    expect(legalCharacterOf({ type: 'BEZWAAR' })).toBeNull();
    expect(submissionKindOf({ type: 'BEZWAAR' })).toBe('BEZWAAR');
    expect(producedDecision(objection)).toBe(objection[1]);
    expect(producedSubmission(objection)).toBe(objection[0]);
  });

  it('reads an appeal as a submission, and the judgment on it as the decision (Awb 8:70)', () => {
    const appeal = [
      { type: 'BEROEP', requests: 'algemene_wet_bestuursrecht#8:70' },
      { type: 'UITSPRAAK', outputs: ['beroep_ontvankelijk', 'beroep_gegrond'] },
    ];
    expect(submissionKindOf(appeal[0])).toBe('BEROEP');
    expect(legalCharacterOf(appeal[0])).toBeNull();
    expect(legalCharacterOf(appeal[1])).toBe('UITSPRAAK');
    expect(producedDecision(appeal)).toBe(appeal[1]);
    expect(producedSubmission(appeal)).toBe(appeal[0]);
  });

  it('gives the decision in the object form terms', () => {
    expect(decisionOf(list)).toEqual({ legal_character: 'BESCHIKKING', decision_type: 'TOEKENNING', procedure_id: undefined });
    expect(decisionOf(object)?.legal_character).toBe('BESCHIKKING');
    expect(decisionOf([list[0]])).toBeNull();
    expect(decisionOf({ submission: { kind: 'AANVRAAG' } })).toBeNull();
  });

  it('is empty without produces', () => {
    expect(producedItems(undefined)).toEqual([]);
    expect(producedDecision(null)).toBeNull();
    expect(producedSubmission(undefined)).toBeNull();
    expect(producedDecision([{ type: 'AANVRAAG' }])).toBeNull();
    expect(decisionOf(null)).toBeNull();
  });

  it('patches a field of the decision, in either form', () => {
    expect(withDecisionField(object, 'decision_type', 'TOEKENNING')).toEqual({ ...object, decision_type: 'TOEKENNING' });
    expect(withDecisionField(undefined, 'decision_type', 'TOEKENNING')).toEqual({ decision_type: 'TOEKENNING' });
    expect(withDecisionField({ decision_type: 'TOEKENNING' }, 'decision_type', null)).toBeUndefined();
    const patched = withDecisionField(list, 'decision_type', 'AFWIJZING') as ProducedItem[];
    expect(patched[1]?.decision_type).toBe('AFWIJZING');
    expect(list[1].decision_type).toBe('TOEKENNING');
    expect((withDecisionField(list, 'legal_character', 'TOETS') as ProducedItem[])[1]?.type).toBe('TOETS');
    expect(withDecisionField(list, 'legal_character', null)).toEqual([list[0]]);
    expect(withDecisionField([list[1]], 'legal_character', null)).toBeUndefined();
  });

  it('leaves a list without a decision alone, until a legal character adds one', () => {
    const application = [list[0]];
    expect(withDecisionField(application, 'decision_type', 'TOEKENNING')).toBe(application);
    expect(withDecisionField(application, 'legal_character', null)).toBe(application);
    expect(withDecisionField(application, 'legal_character', 'BESCHIKKING')).toEqual([list[0], { type: 'BESCHIKKING' }]);
  });
});
