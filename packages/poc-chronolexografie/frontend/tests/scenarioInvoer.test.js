import { readFileSync } from 'fs';
import { load } from 'js-yaml';
import { nextTick } from 'vue';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { formulier, ingeklapt, scenarioVerwijzing, scenarioWaarden } from '../src/scenarioInvoer.js';
import { state } from '../src/state.js';

vi.mock('../src/api.js', () => ({ api: {} }));

// The case as `api.casus` gives it: the YAML, and the articles the app
// derives from the law.
const casus = {
  id: 'bedrijfsparkeervergunning',
  besluit: 'verordening#2',
  beslissing_op_bezwaar: 'awb#7:11',
  uitkomst: 'bedrijfsparkeervergunning_verleend',
  ...load(
    readFileSync(new URL('../../corpus/casussen/bedrijfsparkeervergunning.yaml', import.meta.url), 'utf8'),
  ),
};
const GROEI = 'Een koeriersbedrijf groeit, en de hardheidsclausule na bezwaar';
const aanvraag = { id: 'a1', kind: 'AANVRAAG', basis: 'verordening#2' };
const besluit = { id: 'b1', kind: 'BESCHIKKING', basis: 'verordening#2', refers_to: 'a1', outputs: { bedrijfsparkeervergunning_verleend: true } };

beforeEach(() => {
  Object.assign(state, { casus, scenario: null, grammen: [], andere: [], rechter: [], refusals: [] });
});

describe('the forms in free play', () => {
  it('start empty', () => {
    expect(scenarioWaarden('aanvraag')).toBeNull();
    expect(formulier(() => scenarioWaarden('aanvraag')).waarden('aanvraag')).toEqual({});
  });
});

describe('the record forms of the other cells', () => {
  const ANDERE = { cel: 'x', handelingen: [{ feit: 'wet#1#ander_feit' }] };
  const stappen = (feit) => [
    { stap: 'vastleggen', cel: 'kamer_van_koophandel', feit, invoer: { postcode: '7711 AC' } },
    { stap: 'vastleggen', cel: 'x', feit: 'wet#1#ander_feit' },
  ];
  const FEIT = 'beleid_handelsregister_kamer_van_koophandel#1#vestiging_ingeschreven';
  const cellen = [{ cel: 'kamer_van_koophandel', handelingen: [{ feit: FEIT }, { feit: 'wet#2#tweede' }] }, ANDERE];
  const metStappen = (lijst) => {
    Object.assign(state, {
      casus: { ...casus, scenarios: [{ naam: 'Vouwen', stappen: lijst, eind: 'Klaar.' }], andere_cellen: [] },
      scenario: 'Vouwen',
    });
  };

  it('all stay open in free play', () => {
    expect(ingeklapt(cellen, 'kamer_van_koophandel', FEIT)).toBe(false);
    expect(ingeklapt(cellen, 'x', 'wet#1#ander_feit')).toBe(false);
  });

  it('fold all but the form of the step, in every cell', () => {
    metStappen(stappen(FEIT));
    expect(ingeklapt(cellen, 'kamer_van_koophandel', FEIT)).toBe(false);
    expect(ingeklapt(cellen, 'kamer_van_koophandel', 'wet#2#tweede')).toBe(true);
    expect(ingeklapt(cellen, 'x', 'wet#1#ander_feit')).toBe(true);
  });

  it('follow the scenario to the form of the next step', () => {
    metStappen(stappen(FEIT));
    state.andere = [
      { cel: 'kamer_van_koophandel', grammen: [{ kind: 'EXECUTOGRAM', fact: 'vestiging_ingeschreven' }] },
    ];
    expect(ingeklapt(cellen, 'x', 'wet#1#ander_feit')).toBe(false);
    expect(ingeklapt(cellen, 'kamer_van_koophandel', FEIT)).toBe(true);
  });

  it('stay open at a step on another tab', () => {
    metStappen([{ stap: 'aanvraag' }, ...stappen(FEIT)]);
    expect(ingeklapt(cellen, 'kamer_van_koophandel', FEIT)).toBe(false);
    expect(ingeklapt(cellen, 'x', 'wet#1#ander_feit')).toBe(false);
  });

  it('pass the reference of the record step, for its own cell and fact only', () => {
    expect(scenarioVerwijzing('kamer_van_koophandel', FEIT)).toBeNull();
    metStappen([{ ...stappen(FEIT)[0], verwijst_naar: 'inschrijving_x' }]);
    expect(scenarioVerwijzing('kamer_van_koophandel', FEIT)).toBe('inschrijving_x');
    expect(scenarioVerwijzing('kamer_van_koophandel', 'wet#2#tweede')).toBeNull();
    metStappen(stappen(FEIT));
    expect(scenarioVerwijzing('kamer_van_koophandel', FEIT)).toBeNull();
  });

  it('stay open at a record step whose form is not there', () => {
    metStappen(stappen('wet#9#onbekend'));
    expect(ingeklapt(cellen, 'kamer_van_koophandel', FEIT)).toBe(false);
  });
});

describe('the forms while following a scenario', () => {
  // The Chamber's chronicle: what it knew before the scenario, and the
  // registrations the scenario recorded so far.
  const kamer = (n) => {
    const begin = casus.andere_cellen.find((c) => c.cel === 'kamer_van_koophandel').begin.executogrammen;
    const feit = (e) => e.feit.split('#').at(-1);
    const facts = [...begin.map(feit), ...Array(n).fill(feit(begin[0]))];
    const grammen = facts.map((fact, k) => ({ id: `e${k}`, kind: 'EXECUTOGRAM', fact }));
    return [{ cel: 'kamer_van_koophandel', grammen }];
  };
  const stappen = () => casus.scenarios.find((s) => s.naam === GROEI).stappen;

  beforeEach(() => {
    state.scenario = GROEI;
    state.andere = kamer(1);
  });

  it('fill the application with the example values and the values of the step', () => {
    const vorm = formulier(() => scenarioWaarden('aanvraag'));
    expect(vorm.waarden('aanvraag')).toEqual({ ...casus.voorbeeld.aanvraag, ...stappen()[1].invoer });
  });

  it('fill only the form of the step the scenario is at', () => {
    state.grammen = [aanvraag];
    expect(scenarioWaarden('aanvraag')).toBeNull();
    expect(scenarioWaarden('besluit', { aanvraag: 'a1' })).toEqual(casus.voorbeeld.bevindingen);
    expect(scenarioWaarden('besluit', { aanvraag: 'een andere' })).toBeNull();
  });

  it('fill the second application with its own values', () => {
    state.grammen = [aanvraag, besluit];
    expect(scenarioWaarden('aanvraag')).toMatchObject({ kvk_nummer: '99990055', kenteken: 'ZZ-002-Z' });
  });

  it('keep what the visitor changes, an emptied value too, until the step moves on', async () => {
    const vorm = formulier(() => scenarioWaarden('aanvraag'));
    vorm.zet('aanvraag', 'kenteken', 'XX-999-X');
    vorm.zet('aanvraag', 'naam_aanvrager', undefined);
    expect(vorm.waarden('aanvraag')).toMatchObject({ kenteken: 'XX-999-X', kvk_nummer: '99990055' });
    expect(vorm.waarden('aanvraag')).not.toHaveProperty('naam_aanvrager');
    state.grammen = [aanvraag, besluit];
    await nextTick();
    expect(vorm.waarden('aanvraag').kenteken).toBe('ZZ-002-Z');
  });

  it('fill a record only for the cell and the fact of the step', () => {
    const tweede = { ...aanvraag, id: 'a2' };
    const weigering = { ...besluit, id: 'b2', refers_to: 'a2', outputs: { bedrijfsparkeervergunning_verleend: false } };
    state.grammen = [aanvraag, besluit, tweede, weigering];
    const stap = stappen()[5];
    const doel = { cel: stap.cel, feit: stap.feit };
    expect(scenarioWaarden('vastleggen', doel)).toEqual(stap.invoer);
    expect(scenarioWaarden('vastleggen', { ...doel, cel: 'ggd' })).toBeNull();
  });

  it('start with the registration at the Chamber', () => {
    state.andere = kamer(0);
    const stap = stappen()[0];
    expect(scenarioWaarden('vastleggen', { cel: stap.cel, feit: stap.feit })).toEqual(stap.invoer);
    expect(scenarioWaarden('aanvraag')).toBeNull();
  });
});
