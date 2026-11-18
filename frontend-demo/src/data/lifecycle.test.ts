/**
 * De regels waarmee de demo een zaak aan de Awb-levensloop ophangt (RFC-008).
 *
 * Wat hier bewezen wordt is de verhouding tussen fase en uitkomst: de fase zegt
 * wáár een besluit is, `approved` zegt wát het inhoudt, en die twee mogen niet
 * door elkaar lopen. En dat bezwaar aan de bekendmaking hangt en niet aan het
 * besluit, want daar zat het verschil dat de demo eerder niet liet zien.
 */
import { describe, expect, it } from 'vitest';
import { awbOutcomes, canBeApplied, objectionOpen, reachedStage, reviewedByCaseworker, statusOf } from './lifecycle';

describe('canBeApplied', () => {
  it('lets a beschikking be applied for, but not an aanslag or a non-decision', () => {
    expect(canBeApplied({ legal_character: 'BESCHIKKING', decision_type: 'TOEKENNING' })).toBe(true);
    // Precario, IB, accijns: the government imposes it.
    expect(canBeApplied({ legal_character: 'BESCHIKKING', decision_type: 'AANSLAG' })).toBe(false);
    expect(canBeApplied({ legal_character: 'BESLUIT_VAN_ALGEMENE_STREKKING' })).toBe(false);
    expect(canBeApplied(null)).toBe(false);
  });

  it('reads the decision in a produces list (schema v0.8.1)', () => {
    const produces = [{ type: 'AANVRAAG', requests: 'leerplichtwet_1969#13a' }, { type: 'BESCHIKKING' }];
    expect(canBeApplied(produces)).toBe(true);
    expect(canBeApplied([{ type: 'AANVRAAG' }])).toBe(false);
    expect(canBeApplied([{ type: 'BESCHIKKING', decision_type: 'AANSLAG' }])).toBe(false);
  });
});

describe('reviewedByCaseworker', () => {
  const config = { review_laws: { GEMEENTE_ROTTERDAM: ['algemene_plaatselijke_verordening/terrassen'] } };
  const terras = { service: 'GEMEENTE_ROTTERDAM', law_path: 'algemene_plaatselijke_verordening/terrassen' };

  it('sends an application under a listed law to a caseworker', () => {
    expect(reviewedByCaseworker(terras, config)).toBe(true);
  });

  it('leaves other laws, and the same law at another service, to the law', () => {
    expect(reviewedByCaseworker({ ...terras, law_path: 'algemene_plaatselijke_verordening/exploitatievergunning' }, config)).toBe(false);
    expect(reviewedByCaseworker({ ...terras, service: 'GEMEENTE_AMSTERDAM' }, config)).toBe(false);
    expect(reviewedByCaseworker(terras, {})).toBe(false);
  });
});

const atStage = (stage: string, extra: Record<string, unknown> = {}) => ({
  stageState: { current_stage: stage, accumulated_outputs: {} },
  ...extra,
});

describe('statusOf', () => {
  it('leidt de status af uit de fase', () => {
    expect(statusOf(atStage('AANVRAAG'))).toBe('SUBMITTED');
    expect(statusOf(atStage('BEHANDELING'))).toBe('IN_REVIEW');
    expect(statusOf(atStage('BEKENDMAKING'))).toBe('DECIDED');
    expect(statusOf(atStage('BEZWAAR'))).toBe('DECIDED');
  });

  it('leest een zaak die op BESLUIT wacht niet als besloten', () => {
    // `current_stage` is de fase waar de levensloop vóór staat, niet de fase
    // die hij heeft afgerond. Een zaak op BESLUIT wacht nog op de
    // besluitdatum: de haken van 3:46 en 6:7 hebben niet gevuurd, er is geen
    // besluit. Dit verkeerd lezen zette elke aanvraag die naar een behandelaar
    // gaat meteen als "Afgewezen" op het portaal, omdat `approved` dan null is.
    expect(statusOf(atStage('BESLUIT'))).toBe('IN_REVIEW');
    expect(statusOf(atStage('BESLUIT', { approved: null, decidedAt: null }))).toBe('IN_REVIEW');
  });

  it('maakt van afwijzen geen aparte fase', () => {
    // Toekennen en afwijzen zijn twee uitkomsten van hetzelfde moment; wat het
    // besluit inhoudt staat in `approved`, niet in de fase.
    expect(statusOf(atStage('BEKENDMAKING', { approved: true }))).toBe('DECIDED');
    expect(statusOf(atStage('BEKENDMAKING', { approved: false }))).toBe('DECIDED');
  });

  it('houdt een zaak zonder levensloop bij haar eigen status', () => {
    // Een wet die geen beschikking geeft, of een zaak uit opgeslagen staat van
    // vóór deze verandering: die hoort gewoon te blijven werken.
    expect(statusOf({ status: 'IN_REVIEW' })).toBe('IN_REVIEW');
    expect(statusOf({ status: 'DECIDED' })).toBe('DECIDED');
    expect(statusOf({})).toBe('IN_REVIEW');
  });

  it('kent een ingetrokken zaak', () => {
    expect(statusOf({ ...atStage('BEZWAAR'), withdrawnAt: '2026-03-12T10:00:00Z' })).toBe('WITHDRAWN');
  });
});

describe('reachedStage', () => {
  it('weet welke fase eerder komt', () => {
    expect(reachedStage(atStage('BEKENDMAKING'), 'BESLUIT')).toBe(true);
    expect(reachedStage(atStage('BESLUIT'), 'BEKENDMAKING')).toBe(false);
    expect(reachedStage(atStage('BEZWAAR'), 'BEZWAAR')).toBe(true);
  });

  it('zegt nee als er geen levensloop is', () => {
    expect(reachedStage({}, 'BESLUIT')).toBe(false);
  });
});

describe('objectionOpen', () => {
  it('gaat pas open als de bezwaartermijn loopt', () => {
    // Awb 6:8: de termijn begint de dag ná de bekendmaking. Een besluit dat is
    // genomen maar niet verstuurd, heeft de burger nog niet bereikt.
    expect(objectionOpen(atStage('BESLUIT'))).toBe(false);
    expect(objectionOpen(atStage('BEKENDMAKING'))).toBe(false);
    expect(objectionOpen(atStage('BEZWAAR'))).toBe(true);
  });

  it('gaat weer dicht zodra er bezwaar is gemaakt', () => {
    expect(objectionOpen(atStage('BEZWAAR', { objection: { status: 'PENDING' } }))).toBe(false);
  });
});

describe('awbOutcomes', () => {
  it('leest wat de Awb aan het besluit heeft toegevoegd', () => {
    const c = {
      stageState: {
        current_stage: 'BEZWAAR',
        accumulated_outputs: {
          motivering_vereist: true,
          termijn_weken: 6,
          termijn_startdatum: '2026-03-13',
          termijn_einddatum: '2026-04-23',
        },
      },
    };
    expect(awbOutcomes(c)).toEqual({
      motiveringVereist: true,
      bezwaartermijnWeken: 6,
      bezwaartermijnStart: '2026-03-13',
      bezwaartermijnEinde: '2026-04-23',
    });
  });

  it('geeft niets terug voor een zaak die er nog niet is', () => {
    expect(awbOutcomes(null).bezwaartermijnEinde).toBeNull();
    expect(awbOutcomes(atStage('BESLUIT')).bezwaartermijnEinde).toBeNull();
  });
});

describe('de volgorde waarin de demo een zaak door de fasen brengt', () => {
  // Nagelopen tegen de echte engine over het democorpus: na indienen en
  // toekennen staat de zaak op BEKENDMAKING met de termijn van artikel 6:7
  // erbij, en pas na het bekendmaken komt de einddatum van artikel 6:8 erbij.
  // Wat hier vastligt is wat die fasen voor de rest van de demo betekenen.
  const naToekenning = {
    stageState: {
      current_stage: 'BEKENDMAKING',
      accumulated_outputs: { toegekend: true, motivering_vereist: true, termijn_weken: 6 },
    },
    approved: true,
  };
  const naBekendmaking = {
    stageState: {
      current_stage: 'BEZWAAR',
      accumulated_outputs: {
        ...naToekenning.stageState.accumulated_outputs,
        termijn_startdatum: '2026-03-13',
        termijn_einddatum: '2026-04-23',
      },
    },
    approved: true,
    publishedAt: '2026-03-12T10:00:00Z',
  };

  it('telt een toegekend maar niet bekendgemaakt besluit als besloten', () => {
    // Voor het bord hoort het bij "besloten"; dat het nog de deur uit moet,
    // zegt `publishedAt` en niet de status.
    expect(statusOf(naToekenning)).toBe('DECIDED');
  });

  it('laat nog geen bezwaar toe voordat het besluit is verstuurd', () => {
    expect(objectionOpen(naToekenning)).toBe(false);
    expect(awbOutcomes(naToekenning).bezwaartermijnEinde).toBeNull();
  });

  it('opent bezwaar met een datum zodra het is bekendgemaakt', () => {
    expect(objectionOpen(naBekendmaking)).toBe(true);
    expect(awbOutcomes(naBekendmaking).bezwaartermijnEinde).toBe('2026-04-23');
    expect(awbOutcomes(naBekendmaking).bezwaartermijnWeken).toBe(6);
  });

  // De andere weg: een aanvraag die naar een behandelaar gaat. De levensloop
  // stopt dan vóór BESLUIT en wacht op de besluitdatum, die er pas is als er
  // besloten wordt. Dit is de stand waarin verreweg de meeste zaken staan
  // zodra "alles handmatig beoordelen" aanstaat.
  const wachtOpBehandelaar = {
    stageState: { current_stage: 'BESLUIT', accumulated_outputs: {} },
    pendingInputs: ['besluit_datum'],
    approved: null,
    decidedAt: null,
  };

  it('laat een aanvraag die op een behandelaar wacht niet als besloten zien', () => {
    expect(statusOf(wachtOpBehandelaar)).toBe('IN_REVIEW');
  });

  it('laat op een aanvraag die nog loopt geen bezwaar toe, en geeft er geen termijn bij', () => {
    expect(objectionOpen(wachtOpBehandelaar)).toBe(false);
    expect(awbOutcomes(wachtOpBehandelaar).bezwaartermijnWeken).toBeNull();
  });
});
