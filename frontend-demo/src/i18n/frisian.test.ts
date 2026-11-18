/**
 * Een Fries woord houdt zijn diakriet, ook vooraan een zin.
 *
 * Dit is de fout die geen enkele andere controle ziet. `ôfwiisd` en `útkomst`
 * beginnen met een teken dat op een hoofdletter net zo goed hoort te staan, maar
 * wie een zin begint typt makkelijk de kale letter. De pariteitstest merkt daar
 * niets van, want de string verschilt nog steeds van het Nederlands, en de
 * drempeltest telt hem als vertaald. Hij is alleen fout.
 *
 * Vier vertalers maakten deze fout onafhankelijk van elkaar, en na een eerste
 * correctieronde stonden er nog vijf. Vandaar een test in plaats van nog een
 * veegbeurt: de volgende vertaalronde maakt hem opnieuw.
 *
 * De lijst bevat alleen woordbegin-vormen waarvan de kale variant in het Fries
 * geen bestaand woord is, zodat een terechte treffer niet bestaat. `Un` staat er
 * bewust niet als los patroon in: "Unyk" bestaat niet, maar "Underwiis" is een
 * spelfout waar "Ûnderwiis" hoort, en dat vangt de `Under`-regel al.
 */
import { describe, expect, it } from 'vitest';
import fy from './fy.js';
import nl from './nl.js';

/**
 * Stammen die in het Fries altijd met een diakriet beginnen.
 *
 * Per stam het juiste teken, zodat de foutmelding kan zeggen wat er had moeten
 * staan in plaats van alleen dát er iets mis is.
 */
const NEEDS_DIACRITIC: [string, string, RegExp][] = [
  ['Of', 'Ôf', /(^|[\s("'>])Of(wiis|wiiz|wize|sluting|brekke|hannel|hinne|spraak)/],
  ['Ut', 'Út', /(^|[\s("'>])Ut(fier|kom|kear|zoom|sûndering|stel|lis| namme)/],
  ['Under', 'Ûnder', /(^|[\s("'>])Under(syk|diel|nim|wiis|steun)/],
  ['Un', 'Ûn', /(^|[\s("'>])Un(bekend|tbrek|replik|gegrûn|mooglik)/],
];

describe('het Fries', () => {
  it('adviseert een vorm die bestaat', () => {
    // De melding zegt `"Un…" hoort "Ûn…" te zijn`, dus de vervanging moet
    // evenveel letters houden als de stam. Stond er `Û` tegenover `Un`, dan
    // leest een vertaler dat hij `Ûbekend` moet schrijven in plaats van
    // `Ûnbekend`. De regex vangt dan nog steeds het juiste woord, en het advies
    // eronder is stil fout: precies het soort fout dat deze test zelf hoort te
    // voorkomen.
    for (const [from, to] of NEEDS_DIACRITIC) {
      expect([from, to.length], `${from} -> ${to}`).toEqual([from, from.length]);
    }
  });

  it('houdt zijn diakriet op een hoofdletter', () => {
    const wrong: string[] = [];
    for (const [key, value] of Object.entries(fy)) {
      for (const [from, to, re] of NEEDS_DIACRITIC) {
        if (re.test(value)) wrong.push(`${key}: ${JSON.stringify(value)} — "${from}…" hoort "${to}…" te zijn`);
      }
    }
    expect(wrong, 'Een Fries woord houdt zijn diakriet ook aan het begin van een zin.').toEqual([]);
  });

  it('draagt elke sleutel die het Nederlands heeft', () => {
    // Staat ook in i18n.test.ts, maar daar over elke taal tegelijk. Hier apart,
    // zodat een Friese regressie zich als Fries meldt en niet als "een van de
    // talen".
    expect(Object.keys(nl).filter((k) => !(k in fy))).toEqual([]);
  });
});
