// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { antwoordTekst, ingevuldDoor } from './waarom';

const celNaam = (cel: string) => ({ ggd: 'GGD Voorbeeld' })[cel] ?? cel;

describe('ingevuldDoor', () => {
  it('names a finding, a regulation and a policy with the answer of a cell', () => {
    expect(ingevuldDoor('finding', celNaam, 'Rechtbank Voorbeeld')).toBe('bevinding van Rechtbank Voorbeeld');
    expect(ingevuldDoor({ regulation: { law_id: 'regeling', article: '2' } }, celNaam, 'gemeente Voorbeeld')).toBe(
      'ingevuld door Regeling artikel 2',
    );
    expect(ingevuldDoor({ regulation: { law_id: 'algemene_wet_bestuursrecht', article: '4:2' } }, celNaam, 'gemeente Voorbeeld')).toBe(
      'ingevuld door Awb artikel 4:2',
    );
    expect(
      ingevuldDoor({ answer: { law_id: 'beleid', article: '6', answers: [{ answerer: 'ggd' }, { answerer: 'ggd' }] } }, celNaam, 'gemeente Voorbeeld'),
    ).toBe('ingevuld door Beleid artikel 6, met het antwoord van GGD Voorbeeld');
    expect(ingevuldDoor({ answer: { law_id: 'beleid', article: '6', answers: [] } }, celNaam, 'gemeente Voorbeeld')).toBe(
      'ingevuld door Beleid artikel 6, zonder antwoord van een cel',
    );
    expect(ingevuldDoor(null, celNaam, 'gemeente Voorbeeld')).toBe('niet ingevuld');
  });

  it('says a silent cell leaves it to the authority', () => {
    expect([true, false, null].map((v) => antwoordTekst(v, 'gemeente Voorbeeld'))).toEqual(['ja', 'nee', 'geen antwoord (gemeente Voorbeeld oordeelt)']);
  });
});
