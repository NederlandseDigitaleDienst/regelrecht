// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createApp, nextTick } from 'vue';

// The cell is not loaded here: the page only reads `state`, and no step is
// taken. Nothing is prefilled, so the form asks no source.
vi.mock('../src/api.js', () => ({ api: { voorinvullen: async () => ({}) } }));

const { state } = await import('../src/state.js');
const { zetSessie, wisSessie } = await import('../src/sessie.js');
const { default: AanvragerTab } = await import('../src/components/AanvragerTab.vue');

// FICTIEF: a case with a login step, as the test source tests/bron-inlogstap has one.
const INLOGGEN = {
  veld: 'kvk_nummer',
  sleutel: 'kvk_nummer',
  feit: 'wet_digitale_overheid#15#authenticatie_verricht',
  inlogstap: { cel: 'voorbeeld_erkende_dienst', cel_naam: 'Voorbeelddienst', wachtwoord: 'welkom' },
};
const casus = {
  id: 'voorbeeld',
  titel: 'Voorbeeld',
  cel: 'voorbeeld_autoriteit',
  cel_naam: 'Voorbeeldautoriteit',
  besluit: 'voorbeeldwet#2',
  beslissing_op_bezwaar: 'algemene_wet_bestuursrecht#7:11',
  uitkomst: 'verleend',
  scenarios: [],
};
const vormen = (inloggen) => ({
  aanvraag: { velden: [{ naam: 'kenteken', type: 'string', basis: 'voorbeeldwet#2' }], inloggen },
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
      'voor een nieuwe aanvraag en voor de aanvragen die je al deed',
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
