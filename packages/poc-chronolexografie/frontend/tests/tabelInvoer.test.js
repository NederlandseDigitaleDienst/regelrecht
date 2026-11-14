// @vitest-environment happy-dom
// A field that holds a table, as the application form shows it (VeldInvoer.vue
// with TabelInvoer.vue): the rows in an nldd-table, each row edited in an
// nldd-sheet, rows added and removed. The nldd-* elements are not defined
// here, so the test reads their attributes and fires their events itself.
import { afterEach, describe, expect, it } from 'vitest';
import { createApp, h, nextTick, ref } from 'vue';
import VeldInvoer from '../src/components/VeldInvoer.vue';
import { BESTUUR, RIJEN } from './fixtureTabel.js';

let app = null;
let el = null;

/** Mount the field with `waarde`; returns what it emitted, last first. */
async function mount(waarde, herkomst = '') {
  el = document.createElement('div');
  document.body.append(el);
  const model = ref(waarde);
  const uit = [];
  app = createApp({
    render: () =>
      h(VeldInvoer, {
        veld: BESTUUR,
        modelValue: model.value,
        herkomst,
        'onUpdate:modelValue': (v) => {
          uit.unshift(v);
          model.value = v;
        },
      }),
  });
  app.mount(el);
  await nextTick();
  return uit;
}

const rijen = () => [...el.querySelectorAll('nldd-table > nldd-table-row:not([slot])')];
const cellen = (rij) => [...rij.querySelectorAll('nldd-text-cell')].map((c) => c.getAttribute('text'));
const sheet = () => document.body.querySelector('nldd-sheet');
const knop = (root, tekst) =>
  [...root.querySelectorAll('nldd-button, nldd-icon-button')].find((b) => b.getAttribute('text') === tekst);
const klik = async (b) => {
  b.dispatchEvent(new Event('click'));
  await nextTick();
};
/** The form field of a column in the sheet: the input under its label. */
const invoer = (label) =>
  [...sheet().querySelectorAll('nldd-form-field, nldd-switch-field')].find((f) => f.getAttribute('label') === label);
const typ = async (label, tekst) => {
  const veld = invoer(label).querySelector('nldd-text-field');
  veld.value = tekst;
  veld.dispatchEvent(new Event('input'));
  await nextTick();
};
const bewaar = async () => {
  sheet().querySelector('nldd-form').dispatchEvent(new Event('submit', { cancelable: true }));
  await nextTick();
};

afterEach(() => {
  app?.unmount();
  document.body.innerHTML = '';
});

describe('a table field', () => {
  it('shows its columns as headers and each row as a row of the table', async () => {
    await mount(RIJEN);
    const tabel = el.querySelector('nldd-form-section nldd-table');
    expect(tabel.getAttribute('accessible-label')).toBe('bestuur');
    const kop = el.querySelector('nldd-table-row[slot="header"]');
    expect(cellen(kop)).toEqual(['naam', 'functie', 'sinds', 'vergoeding']);
    expect(rijen().map(cellen)).toEqual([
      ['Lid Een', 'voorzitter', '2026-01-01', '12500'],
      ['Lid Twee', 'penningmeester', '2026-01-15', ''],
    ]);
    expect(el.querySelector('nldd-form-section').getAttribute('supporting-text')).toBe('voorbeeld_inschrijving#1');
  });

  it('shows where a prefilled table comes from under it', async () => {
    await mount(RIJEN, 'Ingevuld uit Voorbeeldregister (bestuur bij nummer).');
    expect(el.querySelector('nldd-form-section nldd-text').textContent).toBe(
      'Ingevuld uit Voorbeeldregister (bestuur bij nummer).',
    );
  });

  it('says so when it holds no rows', async () => {
    await mount(undefined);
    expect(rijen()).toEqual([]);
    expect(el.querySelector('nldd-table [slot="empty"]').getAttribute('text')).toBe('Nog geen rijen');
  });

  it('edits a row in a sheet, with a field per column', async () => {
    const uit = await mount(RIJEN);
    expect(sheet().hasAttribute('open')).toBe(false);
    await klik(knop(rijen()[0], 'Bewerk'));
    expect(sheet().hasAttribute('open')).toBe(true);
    const titel = sheet().querySelector('nldd-top-title-bar');
    expect(titel.getAttribute('text')).toBe('Lid Een, voorzitter, 2026-01-01 bewerken');
    expect(titel.getAttribute('dismiss-text')).toBe('Annuleer');
    expect(sheet().querySelector(`#${titel.getAttribute('collapse-anchor')}`).getAttribute('text')).toBe(
      titel.getAttribute('text'),
    );
    expect(invoer('vergoeding').querySelector('nldd-text-field').getAttribute('value')).toBe('12500');
    await typ('vergoeding', '15000');
    expect(uit).toEqual([]);
    await bewaar();
    expect(uit[0]).toEqual([{ ...RIJEN[0], vergoeding: 15000 }, RIJEN[1]]);
    expect(sheet().hasAttribute('open')).toBe(false);
  });

  it('keeps the table as it was when the sheet closes without saving', async () => {
    const uit = await mount(RIJEN);
    await klik(knop(rijen()[0], 'Bewerk'));
    await typ('naam', 'Iemand anders');
    sheet().dispatchEvent(new Event('close'));
    await nextTick();
    expect(sheet().hasAttribute('open')).toBe(false);
    expect(uit).toEqual([]);
    expect(cellen(rijen()[0])[0]).toBe('Lid Een');
  });

  it('adds a row, whose empty cells hold null', async () => {
    const uit = await mount(RIJEN);
    await klik(knop(el, 'Rij toevoegen'));
    expect(sheet().querySelector('nldd-top-title-bar').getAttribute('text')).toBe('Rij toevoegen aan bestuur');
    await typ('naam', 'Lid Drie');
    await bewaar();
    expect(uit[0]).toEqual([
      ...RIJEN,
      { naam: 'Lid Drie', functie: null, sinds: null, vergoeding: null },
    ]);
  });

  it('removes a row', async () => {
    const uit = await mount(RIJEN);
    await klik(knop(rijen()[0], 'Verwijder'));
    expect(uit[0]).toEqual([RIJEN[1]]);
    expect(rijen().map((r) => cellen(r)[0])).toEqual(['Lid Twee']);
  });

  it('asks every column type with its own input', async () => {
    const veld = {
      naam: 'regels',
      type: 'array',
      mag_leeg: false,
      basis: 'r#1',
      kolommen: [
        { naam: 'tekst', type: 'string' },
        { naam: 'getal', type: 'number' },
        { naam: 'bedrag', type: 'amount' },
        { naam: 'datum', type: 'date' },
        { naam: 'ja_nee', type: 'boolean' },
      ],
    };
    el = document.createElement('div');
    document.body.append(el);
    const uit = [];
    app = createApp({
      render: () => h(VeldInvoer, { veld, modelValue: [], 'onUpdate:modelValue': (v) => uit.unshift(v) }),
    });
    app.mount(el);
    await nextTick();
    await klik(knop(el, 'Rij toevoegen'));
    const soort = (label) => invoer(label).firstElementChild.tagName.toLowerCase();
    expect(['tekst', 'getal', 'bedrag', 'datum', 'ja nee'].map(soort)).toEqual([
      'nldd-text-field',
      'nldd-text-field',
      'nldd-text-field',
      'nldd-date-field',
      'nldd-dropdown',
    ]);
    // Every cell may stay empty: yes or no is a choice of three.
    const select = invoer('ja nee').querySelector('select');
    expect([...select.options].map((o) => o.value)).toEqual(['', 'ja', 'nee']);
    await typ('getal', '2,5');
    await typ('bedrag', '1250');
    const datum = invoer('datum').querySelector('nldd-date-field');
    datum.dispatchEvent(new CustomEvent('change', { detail: { value: '2026-03-01' } }));
    select.value = 'ja';
    select.dispatchEvent(new Event('change'));
    await nextTick();
    await bewaar();
    expect(uit[0]).toEqual([{ tekst: null, getal: 2.5, bedrag: 1250, datum: '2026-03-01', ja_nee: true }]);
  });
});

describe('a yes-or-no column', () => {
  it('goes back to empty', async () => {
    const veld = { naam: 'r', type: 'array', mag_leeg: true, basis: 'r#1', kolommen: [{ naam: 'actief', type: 'boolean' }] };
    el = document.createElement('div');
    document.body.append(el);
    const uit = [];
    app = createApp({
      render: () => h(VeldInvoer, { veld, modelValue: [{ actief: true }], 'onUpdate:modelValue': (v) => uit.unshift(v) }),
    });
    app.mount(el);
    await nextTick();
    expect(cellen(rijen()[0])).toEqual(['ja']);
    await klik(knop(rijen()[0], 'Bewerk'));
    const select = invoer('actief').querySelector('select');
    expect(select.value).toBe('ja');
    select.value = '';
    select.dispatchEvent(new Event('change'));
    await nextTick();
    await bewaar();
    expect(uit[0]).toEqual([{ actief: null }]);
  });
});
