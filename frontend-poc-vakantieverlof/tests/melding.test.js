import { describe, expect, it } from 'vitest';
import { isEngels, melding } from '../src/melding.js';

// De meldingen zoals de cel ze geeft (packages/cel: cell.rs, chronicle.rs),
// met verzonnen ids. Elk patroon in melding.js heeft hier een geval.
const ids = {
  besluit: '019a0000-0000-7000-8000-00000000aaaa',
  vervanger: '019a0000-0000-7000-8000-00000000bbbb',
  beslissing: '019a0000-0000-7000-8000-00000000cccc',
};

const algemeen = 'Wat er misging, staat onder Details, in het Engels.';

describe('melding', () => {
  it('zegt dat een besluit maar één keer wordt vervangen, en door welk', () => {
    const fout =
      `refused: the decision of algemene_wet_bestuursrecht#7:11 would take a decision in the place of '${ids.besluit}', ` +
      `which chronolexogram '${ids.vervanger}' already replaces (taken by '${ids.beslissing}')`;
    expect(melding(fout)).toBe(
      'Besluit 0000aaaa is na een eerder bezwaar al vervangen door besluit 0000bbbb. Een besluit kan maar één keer worden vervangen. Er is niets vastgelegd.',
    );
  });

  it('zegt dat de kroniek alleen vooruit loopt', () => {
    const fout =
      `refused: chronolexogram '${ids.besluit}' is dated 2026-10-20, before the last chronolexogram of the chronicle ` +
      `('${ids.vervanger}', 2026-11-01): the chronicle does not go back in time`;
    expect(melding(fout)).toContain('vóór de laatste stap in de kroniek (2026-11-01)');
  });

  it('zegt dat de reductie van de school op die dag nog niet geldt', () => {
    const fout =
      "setup: open term 'verlof_al_verleend_in_schooljaar' of leerplichtwet_1969#13a is filled by a reduction in " +
      'corpus/cells/obs_de_klimop, but none is in force on 2026-07-01; the cell does not take it as a finding of the deciding authority';
    expect(melding(fout)).toBe(
      'De school beantwoordt "verlof al verleend in schooljaar" uit haar kroniek, maar op 2026-07-01 geldt haar reductie nog niet. Kies een latere speeldatum.',
    );
  });

  it('zegt dat het besluit de school niet om dat begrip vroeg', () => {
    const fout = `refused: the decision '${ids.besluit}' did not ask the cell of 'obs_de_klimop' for 'verlofdagen'`;
    expect(melding(fout)).toBe(
      'Besluit 0000aaaa vroeg de school niet om "verlofdagen", dus er is geen antwoord om uit te leggen.',
    );
  });

  it('zegt dat er al besloten is', () => {
    const fout = `refused: the application '${ids.besluit}' is already decided on, by chronolexogram '${ids.vervanger}'`;
    expect(melding(fout)).toBe('Hierop is al besloten.');
  });

  it('noemt het veld dat de aanvraag mist', () => {
    const fout = `refused: the application '${ids.besluit}' lacks 'laatste_verlofdag', which the decision of leerplichtwet_1969#13a needs (Awb 4:5)`;
    expect(melding(fout)).toContain('"laatste verlofdag"');
  });

  it('noemt de bevinding die het hoofd nog moet geven', () => {
    const fout =
      "refused: the decision of leerplichtwet_1969#13a asks 'specifieke_aard_beroep_ouder' of the deciding authority (leerplichtwet_1969#11)";
    expect(melding(fout)).toContain('"specifieke aard beroep ouder" nog niet beoordeeld');
  });

  it('noemt een waarde die niet past, met en zonder non-nullable', () => {
    for (const soort of ['non-nullable Date', 'Boolean']) {
      const fout = `refused: 'eerste_verlofdag' is "morgen", which does not fit a ${soort} (leerplichtwet_1969#13a)`;
      expect(melding(fout)).toContain('"eerste verlofdag" past niet');
    }
  });

  it('noemt een veld dat de wet niet vraagt', () => {
    const fout = "refused: the law does not ask 'bestemming' in the AANVRAAG of leerplichtwet_1969#13a";
    expect(melding(fout)).toBe('De wet vraagt niet om "bestemming" bij de aanvraag.');
    expect(melding(fout.replace('AANVRAAG', 'BEZWAAR'))).toContain('bij het bezwaar');
  });

  it('valt terug op een algemene zin', () => {
    expect(melding('engine: something else')).toBe(algemeen);
    expect(melding('De server gaf geen antwoord.')).toBe('Wat er misging, staat onder Details.');
  });
});

describe('isEngels', () => {
  it('herkent een melding van de cel of de engine', () => {
    expect(isEngels('refused: x')).toBe(true);
    expect(isEngels('engine: x')).toBe(true);
    expect(isEngels('Failed to fetch')).toBe(true);
    expect(isEngels('De server gaf geen antwoord.')).toBe(false);
  });
});
