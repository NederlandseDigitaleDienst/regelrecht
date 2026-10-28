import { describe, expect, it } from 'vitest';
import { melding } from '../src/melding.js';
import { state } from '../src/state.js';

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

  it('zegt welke bevinding het bestuursorgaan nog niet gaf', () => {
    // De melding van de engine (packages/engine/src/service.rs).
    const engine =
      "engine: Resolution error: Required open term 'verlof_valt_buiten_schoolvakanties' on leerplichtwet_1969#13a " +
      'is decided per case by het hoofd (artikel 13a, eerste lid), and no judgement was given for this case';
    state.casus = { cel_naam: 'OBS De Klimop', rol: 'het hoofd' };
    expect(melding(engine)).toBe(
      'Het hoofd heeft "verlof valt buiten schoolvakanties" nog niet beoordeeld, en het besluit heeft die bevinding nodig.',
    );
    state.casus = null;
  });

  it('zegt dat het beleid een oordeel voor deze aanvraag aan het bestuursorgaan laat', () => {
    // De parkeerkaart zonder advies van de GGD: het beleid laat het oordeel
    // aan het college, met een eigen open begrip.
    const engine =
      "engine: Resolution error: Required open term 'langdurige_loopbeperking_volgens_college' on " +
      'beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#6 is decided per case by het college ' +
      'van burgemeester en wethouders van de gemeente Wilgendaal, and no judgement was given for this case';
    state.casus = { cel_naam: 'gemeente Wilgendaal', rol: 'het college van burgemeester en wethouders' };
    expect(
      melding(
        `the policy leaves 'langdurige_loopbeperking_volgens_college' to the authority for this case: ${engine}`,
      ),
    ).toBe(
      'De regels laten "langdurige loopbeperking volgens college" voor deze aanvraag aan het college van burgemeester en wethouders over. Geef die bevinding.',
    );
    state.casus = null;
  });

  it('zegt dat de reductie van de cel op die dag nog niet geldt', () => {
    const fout =
      "setup: open term 'verlof_al_verleend_in_schooljaar' of leerplichtwet_1969#13a is filled by a reduction in " +
      'corpus/cells/obs_de_klimop, but none is in force on 2026-07-01; the cell does not take it as a finding of the deciding authority';
    expect(melding(fout)).toBe(
      'De cel beantwoordt "verlof al verleend in schooljaar" uit haar kroniek, maar op 2026-07-01 geldt haar reductie nog niet. Kies een latere speeldatum.',
    );
  });

  it('zegt dat het besluit de cel niet om dat begrip vroeg', () => {
    const fout = `refused: the decision '${ids.besluit}' did not ask the cell of 'obs_de_klimop' for 'verlofdagen'`;
    expect(melding(fout)).toBe(
      'Besluit 0000aaaa vroeg de cel niet om "verlofdagen", dus er is geen antwoord om uit te leggen.',
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

  it('noemt de bevinding die het bestuursorgaan nog moet geven', () => {
    const fout =
      "refused: the decision of leerplichtwet_1969#13a asks 'specifieke_aard_beroep_ouder' of the deciding authority (leerplichtwet_1969#11)";
    expect(melding(fout)).toContain('"specifieke aard beroep ouder" nog niet gegeven');
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

  it('zegt dat er eerst een casus gekozen moet worden', () => {
    expect(melding('no case is chosen: choose one first')).toBe('Er is geen casus gekozen. Kies een casus.');
  });

  it('zegt dat de cel in deze browser niet laadt', () => {
    expect(melding('the cell did not load: Failed to fetch')).toContain('De cel kon in deze browser niet laden.');
  });

  it('zegt dat de cel vastliep, en dat een herlaad verder gaat', () => {
    expect(melding('the cell stopped: unreachable')).toContain('De cel is vastgelopen.');
  });

  it('valt terug op een algemene zin', () => {
    expect(melding('engine: something else')).toBe(algemeen);
    expect(melding('Failed to fetch')).toBe(algemeen);
  });
});
