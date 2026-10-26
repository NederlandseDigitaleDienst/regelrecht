import { nextTick } from 'vue';
import { beforeEach, describe, expect, it } from 'vitest';
import { state } from '../src/state.js';
import { nietGeweten, stilBegrippen } from '../src/stil.js';

const veld = { naam: 'verlof_valt_buiten_schoolvakanties', type: 'boolean', basis: 'leerplichtwet_1969#13a' };

beforeEach(() => {
  state.casus = { cel_naam: 'OBS De Klimop', rol: 'het hoofd' };
  state.datum = '2027-01-18';
  state.grammen = [];
});

describe('stilBegrippen', () => {
  it('meldt een begrip één keer, ook als de voorvertoning het opnieuw meldt', () => {
    const { stil, meldStil } = stilBegrippen({});
    meldStil('o', [veld]);
    meldStil('o', [veld]);
    expect(stil.o).toHaveLength(1);
  });

  it('vergeet het begrip en het oordeel als de speeldatum verandert', async () => {
    const oordelen = { o: { verlof_valt_buiten_schoolvakanties: true, specifieke_aard_beroep_ouder: true } };
    const { stil, meldStil } = stilBegrippen(oordelen);
    meldStil('o', [veld]);
    state.datum = '2027-01-19';
    await nextTick();
    expect(stil.o).toBeUndefined();
    expect(oordelen.o).toEqual({ specifieke_aard_beroep_ouder: true });
  });
});

describe('nietGeweten', () => {
  it('zegt dat de cel het niet weet en wie oordeelt', () => {
    expect(nietGeweten(veld)).toBe(
      'leerplichtwet_1969#13a · OBS De Klimop weet dit niet voor deze aanvraag; het hoofd oordeelt.',
    );
  });

  it('zegt dat het beleid het oordeel voor deze aanvraag aan het bestuursorgaan laat', () => {
    expect(nietGeweten({ ...veld, beleid: true })).toBe(
      'leerplichtwet_1969#13a · Het beleid laat dit voor deze aanvraag aan het hoofd over.',
    );
  });
});
