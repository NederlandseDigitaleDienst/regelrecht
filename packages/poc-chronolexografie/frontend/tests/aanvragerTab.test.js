// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createApp, nextTick } from 'vue';

// The cell is not loaded here. A submitted application goes into a chronicle
// of this file (`kroniek`), which the page fetches again after the step.
// Nothing is prefilled, so the form asks no source.
const kroniek = [];
vi.mock('../src/api.js', () => ({
  api: {
    voorinvullen: async () => ({}),
    indienen: async (artikel, velden, datum) => {
      kroniek.push({ id: `aanvr${kroniek.length}`, kind: 'AANVRAAG', basis: artikel, recorded_on: datum, fields: velden });
      return {};
    },
    kroniek: async () => ({ grammen: [...kroniek], andere: [] }),
    vormen: async () => ({ indieningen: { 'voorbeeldwet#1': vormen(KEUZE).aanvraag }, besluiten: {} }),
    bewaren: async () => ({}),
  },
}));

const { state } = await import('../src/state.js');
const { zetKeuze, zetSessie, wisSessie } = await import('../src/sessie.js');
const { default: AanvragerTab } = await import('../src/components/AanvragerTab.vue');

// FICTIEF: a case with a login step, as the test source tests/bron-inlogstap has one.
const INLOGGEN = {
  veld: 'kvk_nummer',
  sleutel: 'kvk_nummer',
  feit: 'wet_digitale_overheid#15#authenticatie_verricht',
  inlogstap: { cel: 'voorbeeld_erkende_dienst', cel_naam: 'Voorbeelddienst', wachtwoord: 'welkom' },
};
// FICTIEF: a case without a login step, where the applicant picks a record.
const KEUZE = {
  veld: 'kvk_nummer',
  sleutel: 'kvk_nummer',
  feit: 'wet_digitale_overheid#15#authenticatie_verricht',
  keuzes: [{ waarde: '99970011', cel: 'voorbeeld_erkende_dienst', vastgelegd_op: '2026-01-01', velden: {} }],
};
const casus = {
  id: 'voorbeeld',
  artikel: 'voorbeeldwet#1',
  titel: 'Voorbeeld',
  cel: 'voorbeeld_autoriteit',
  cel_naam: 'Voorbeeldautoriteit',
  besluit: 'voorbeeldwet#2',
  beslissing_op_bezwaar: 'algemene_wet_bestuursrecht#7:11',
  uitkomst: 'verleend',
  // A scenario of lines only has no guide, so every form starts from the
  // example values (scenarioInvoer.js): it is at an application step all along.
  scenarios: [{ naam: 'Alleen tekst', stappen: ['De aanvrager vraagt nog eens aan.'] }],
};
const vormen = (inloggen) => ({
  aanvraag: {
    velden: [
      { naam: 'kvk_nummer', type: 'string', basis: 'voorbeeldwet#2' },
      { naam: 'kenteken', type: 'string', basis: 'voorbeeldwet#2' },
    ],
    inloggen,
  },
  bezwaar: { velden: [] },
});
const aanvraag = (id, kvk) => ({
  id,
  kind: 'AANVRAAG',
  basis: 'voorbeeldwet#1',
  recorded_on: '2026-01-05',
  fields: { kvk_nummer: kvk },
});
const besluit = (id, op) => ({
  id,
  kind: 'BESLUIT',
  basis: 'voorbeeldwet#2',
  refers_to: op,
  recorded_on: '2026-01-06',
  outputs: { verleend: true },
});

let app = null;
let el = null;
async function mount() {
  el = document.createElement('div');
  document.body.append(el);
  app = createApp(AanvragerTab);
  app.mount(el);
  for (let i = 0; i < 5; i++) await nextTick();
}
const knoppen = () => [...el.querySelectorAll('nldd-button')].map((b) => b.getAttribute('text'));

beforeEach(() => {
  Object.assign(state, {
    casus,
    datum: '2026-01-10',
    grammen: [],
    andere: [],
    rechter: [],
    vormen: vormen(INLOGGEN),
    scenario: null,
    fout: '',
  });
  kroniek.length = 0;
  wisSessie(casus.id);
});

afterEach(() => {
  app?.unmount();
  document.body.innerHTML = '';
});

describe('the login block', () => {
  it('stands above the form and Mijn aanvragen, in a section of its own', async () => {
    await mount();
    const [eerste, tweede] = el.children;
    expect(eerste.tagName.toLowerCase()).toBe('nldd-simple-section');
    expect(eerste.querySelector('nldd-title').getAttribute('text')).toBe('Niet ingelogd');
    expect(eerste.querySelector('nldd-title').getAttribute('supporting-text')).toContain(
      'voor een nieuwe aanvraag en voor de aanvragen die je al indiende',
    );
    expect(tweede.tagName.toLowerCase()).toBe('nldd-one-half-one-half-section');
    expect(tweede.querySelector('nldd-button[text="Inloggen"]')).toBeNull();
  });

  it('is absent in a case without a login', async () => {
    state.vormen = vormen(undefined);
    await mount();
    expect(el.querySelector('nldd-simple-section')).toBeNull();
    expect(knoppen()).toContain('Aanvraag indienen');
  });
});

describe('the form for a new application', () => {
  it('is open while the applicant has no application in behandeling', async () => {
    zetSessie(casus.id, { gebruikersnaam: 'bakker', claims: { kvk_nummer: '99970011' } });
    state.grammen = [aanvraag('a1', '99970011'), besluit('b1', 'a1'), aanvraag('a2', '11111111')];
    await mount();
    expect(knoppen()).toContain('Aanvraag indienen');
    expect(knoppen()).not.toContain('Nieuwe aanvraag');
  });

  it('stays closed behind a button while one of theirs is in behandeling', async () => {
    zetSessie(casus.id, { gebruikersnaam: 'bakker', claims: { kvk_nummer: '99970011' } });
    state.grammen = [aanvraag('aaaa0001', '99970011')];
    await mount();
    expect(knoppen()).not.toContain('Aanvraag indienen');
    const melding = el.querySelector('nldd-inline-dialog');
    expect(melding.getAttribute('text')).toBe('Je aanvraag is in behandeling');
    expect(melding.getAttribute('supporting-text')).toContain('Aanvraag aaaa0001 wacht');

    el.querySelector('nldd-button[text="Nieuwe aanvraag"]').click();
    await nextTick();
    expect(knoppen()).toContain('Aanvraag indienen');
  });

  it('counts every application in behandeling in a case without a login', async () => {
    state.vormen = vormen(undefined);
    state.grammen = [aanvraag('a1', undefined)];
    await mount();
    expect(knoppen()).not.toContain('Aanvraag indienen');
    expect(knoppen()).toContain('Nieuwe aanvraag');
  });
});

describe('the choice under Ingelogd als, in a case without a login step', () => {
  const kies = async (waarde) => {
    const select = el.querySelector('nldd-simple-section select');
    select.value = waarde;
    select.dispatchEvent(new Event('change'));
    for (let i = 0; i < 3; i++) await nextTick();
  };
  const indienen = async () => {
    el.querySelector('nldd-button[text="Aanvraag indienen"]').click();
    for (let i = 0; i < 10; i++) await nextTick();
    await new Promise((r) => setTimeout(r));
    await nextTick();
  };

  it('stays after a submit, so the form closes on the application of whoever chose', async () => {
    state.vormen = vormen(KEUZE);
    await mount();
    await kies('99970011');
    await indienen();
    expect(kroniek.map((a) => a.fields.kvk_nummer)).toEqual(['99970011']);
    expect(el.querySelector('nldd-simple-section select').value).toBe('99970011');
    expect(knoppen()).not.toContain('Aanvraag indienen');
    expect(knoppen()).toContain('Nieuwe aanvraag');
  });

  it('gives way to what the applicant types in the login field of the form', async () => {
    state.vormen = vormen(KEUZE);
    state.grammen = [aanvraag('a1', '99970011'), aanvraag('a2', '22222222')];
    zetKeuze(casus.id, '99970011');
    await mount();
    el.querySelector('nldd-button[text="Nieuwe aanvraag"]').click();
    for (let i = 0; i < 3; i++) await nextTick();
    // The other number has an application in behandeling too; typing it
    // keeps the form open, and the form holds what was typed.
    const veld = el.querySelector('nldd-one-half-one-half-section nldd-text-field');
    veld.value = '22222222';
    veld.dispatchEvent(new Event('input'));
    for (let i = 0; i < 3; i++) await nextTick();
    expect(knoppen()).toContain('Aanvraag indienen');
    expect(el.querySelector('nldd-simple-section select').value).toBe('');

    // Submitted, a number that is no choice does not become the login.
    await indienen();
    expect(kroniek.map((a) => a.fields.kvk_nummer)).toEqual(['22222222']);
    expect(el.querySelector('nldd-simple-section select').value).toBe('99970011');
  });

  it('closes the form again when someone else logs in', async () => {
    state.vormen = vormen({ ...KEUZE, keuzes: [...KEUZE.keuzes, { ...KEUZE.keuzes[0], waarde: '22222222' }] });
    state.grammen = [aanvraag('a1', '99970011'), aanvraag('a2', '22222222')];
    zetKeuze(casus.id, '99970011');
    await mount();
    el.querySelector('nldd-button[text="Nieuwe aanvraag"]').click();
    for (let i = 0; i < 3; i++) await nextTick();
    expect(knoppen()).toContain('Aanvraag indienen');
    await kies('22222222');
    expect(knoppen()).not.toContain('Aanvraag indienen');
    expect(knoppen()).toContain('Nieuwe aanvraag');
  });

  it('stays when the tab opens again', async () => {
    state.vormen = vormen(KEUZE);
    await mount();
    await kies('99970011');
    app.unmount();
    document.body.innerHTML = '';
    state.grammen = [aanvraag('a1', '99970011')];
    await mount();
    expect(el.querySelector('nldd-simple-section select').value).toBe('99970011');
    expect(knoppen()).toContain('Nieuwe aanvraag');
  });
});

describe('a scenario at an application step', () => {
  it('opens the form, also while an application is in behandeling', async () => {
    state.vormen = vormen(undefined);
    state.grammen = [aanvraag('a1', undefined)];
    state.scenario = 'Alleen tekst';
    await mount();
    expect(knoppen()).toContain('Aanvraag indienen');
    expect(knoppen()).not.toContain('Nieuwe aanvraag');
  });
});
