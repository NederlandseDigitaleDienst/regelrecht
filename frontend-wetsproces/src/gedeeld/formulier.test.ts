// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { aanvraagformulier, aanvraagVelden, beginwaarden, schermveld, tegenBesluitVelden } from './formulier';
import type { Indieningsvorm } from './types';

// A made-up application, not of any case: the form follows from what the
// server derives from the law and what the UI spec says of each field.
const vorm: Indieningsvorm = {
  soort: 'AANVRAAG',
  basis: 'voorbeeldwet#3',
  besluit: 'voorbeeldwet#3',
  cel: 'voorbeeldcel',
  velden: [
    { naam: 'nummer_aanvrager', type: 'string', mag_leeg: false, basis: 'voorbeeldwet#3' },
    { naam: 'wil_een_vergunning', type: 'boolean', mag_leeg: false, basis: 'voorbeeldwet#3' },
    { naam: 'ondertekend', type: 'boolean', mag_leeg: true, basis: 'algemene_wet#4:2' },
    { naam: 'kanaal', type: 'string', mag_leeg: true, basis: 'voorbeeldwet#3' },
    { naam: 'dagtekening', type: 'date', mag_leeg: true, basis: 'algemene_wet#4:2' },
  ],
  systeem: { nummer_aanvrager: {}, kanaal: { waarde: 'portaal' } },
  verborgen: ['kanaal'],
};

describe('aanvraagformulier', () => {
  it('asks what the law asks, but not what the system fills or the UI spec hides', () => {
    const f = aanvraagformulier(vorm, {}, '999990019');
    expect(f.invullen.map((s) => s.veld.naam)).toEqual(['wil_een_vergunning', 'ondertekend', 'dagtekening']);
    // The login fills the number and the form shows it; the channel is hidden.
    expect(f.doorSysteem).toEqual([
      expect.objectContaining({ label: 'Nummer aanvrager', waarde: '999990019', uitInlog: true }),
    ]);
  });

  it('names a field after the regelwerk, unless the UI spec gives it a label and help', () => {
    const f = aanvraagformulier(vorm, { wil_een_vergunning: { label: 'Ik vraag een vergunning aan', help: 'Zet dit aan.' } }, null);
    expect(f.invullen.map((s) => [s.label, s.help])).toEqual([
      ['Ik vraag een vergunning aan', 'Zet dit aan.'],
      ['Ondertekend', ''],
      ['Dagtekening', ''],
    ]);
  });

  it('shows every field of the law when the case has no UI spec', () => {
    const kaal = { ...vorm, systeem: undefined, verborgen: undefined };
    const f = aanvraagformulier(kaal, undefined, null);
    expect(f.invullen.map((s) => s.veld.naam)).toEqual(vorm.velden.map((v) => v.naam));
    expect(f.doorSysteem).toEqual([]);
  });
});

describe('aanvraagVelden', () => {
  it('sends what the system knows and leaves out the login field and the empty ones', () => {
    const velden = aanvraagVelden(vorm, { wil_een_vergunning: true, ondertekend: undefined, nummer_aanvrager: 'x' });
    // The server fills the login field itself and refuses it from the request.
    expect(velden).toEqual({ wil_een_vergunning: true, kanaal: 'portaal' });
  });
});

describe('beginwaarden', () => {
  it('starts a yes-or-no the law requires at no, and leaves the rest empty', () => {
    const velden = vorm.velden.map((v) => schermveld(v));
    expect(beginwaarden(velden)).toEqual({ wil_een_vergunning: false });
  });
});

describe('tegenBesluitVelden', () => {
  it('leaves out what the decision already holds', () => {
    const bezwaar: Indieningsvorm = {
      ...vorm,
      velden: [
        { naam: 'gronden', type: 'string', mag_leeg: true },
        { naam: 'termijn_einddatum', type: 'date', mag_leeg: false },
      ],
    };
    const velden = tegenBesluitVelden(bezwaar, { outputs: { termijn_einddatum: '2026-11-24' } }, {});
    expect(velden.map((s) => s.veld.naam)).toEqual(['gronden']);
  });
});

describe('schermveld', () => {
  it('names the article a field comes from in words, never by its key', () => {
    const veld = vorm.velden.find((v) => v.naam === 'dagtekening')!;
    expect(schermveld(veld).veld.basis).toBe('Algemene wet artikel 4:2');
    expect(schermveld({ ...veld, basis: 'algemene_wet_bestuursrecht#4:2' }).veld.basis).toBe('Awb artikel 4:2');
    // The field itself is left as it is: the value still goes under its own name.
    expect(veld.basis).toBe('algemene_wet#4:2');
  });
});
