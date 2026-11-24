// A table from the chronicle, on a case of its own: a municipality counts the
// parking places per district that an applicant asks about, and a survey
// office recorded the counts. The nldd-* elements are not defined in this
// test, so it reads the attributes Vue sets on them.
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { alsRijen, kolommen } from './tabel';
import TabelUitKroniek from './TabelUitKroniek.vue';
import type { Tabel, Tabelrij } from './types';

const tabel: Tabel = {
  naam: 'T1',
  element: 'plaatsen_per_wijk',
  rol: 'behandelaar',
  veld: 'plaatsen',
  kolommen: [
    { name: 'wijk', label: 'Wijk' },
    { name: 'aantal', label: 'Aantal' },
  ],
  herkomst: [{ fact: 'telling#3#telling_ontvangen', match: ['wijk'] }],
  label: 'Plaatsen per wijk',
};

const rijen: Tabelrij[] = [
  {
    waarden: { wijk: 'Noord', aantal: 3 },
    herkomst: [
      { cel: 'gemeente', wat: 'regeling#2', stuk: 'a1', op: '2026-03-06' },
      { cel: 'bureau', wat: 'telling#3#telling_ontvangen', stuk: 't2', op: '2026-02-02' },
    ],
  },
  { waarden: { wijk: 'Zuid', aantal: 2 }, herkomst: [{ cel: 'gemeente', wat: 'regeling#2', stuk: 'a1', op: '2026-03-06' }], zonder_herkomst: ['telling#3#telling_ontvangen'] },
];

const namen: Record<string, string> = { gemeente: 'Gemeente Wilgendaal', bureau: 'Telbureau' };

function teken(props: Record<string, unknown> = {}) {
  return mount(TabelUitKroniek, { props: { tabel, rijen, celNaam: (c: string) => namen[c] ?? c, ...props } });
}

describe('TabelUitKroniek', () => {
  it('shows a column per key the UI spec names, with its label, and a column for where a row comes from', () => {
    const w = teken();
    const kop = w.find('nldd-table-row[slot="header"]').findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(kop).toEqual(['Wijk', 'Aantal', 'Herkomst']);
    expect(w.find('nldd-title').attributes('text')).toBe('Plaatsen per wijk');
  });

  it('puts per row the values and a tag per record it comes from: the cell, what it records and the day', () => {
    const w = teken();
    const [, noord, zuid] = w.findAll('nldd-table-row');
    expect(noord!.findAll('nldd-text-cell').map((c) => c.attributes('text'))).toEqual(['Noord', '3']);
    const tags = noord!.findAll('nldd-tag').map((t) => t.attributes('text'));
    expect(tags).toEqual(['Gemeente Wilgendaal', 'Telbureau']);
    expect(noord!.findAll('nldd-tag')[1]!.attributes('accessible-label')).toBe('Telbureau · telling ontvangen · 2 februari 2026');
    // A row that no record of the named fact matched says so, rather than leaving the source out.
    expect(zuid!.findAll('nldd-tag').map((t) => t.attributes('text'))).toEqual(['Gemeente Wilgendaal', 'Geen telling ontvangen']);
  });

  it('says why a table has no rows', () => {
    const w = teken({ rijen: [], leeg: 'de indiening houdt niets onder plaatsen' });
    expect(w.find('nldd-table').exists()).toBe(false);
    expect(w.find('nldd-inline-dialog').attributes('supporting-text')).toBe('de indiening houdt niets onder plaatsen');
  });

  it('shows a list of rows a form holds read-only, without sources, with every key as a column', () => {
    const r = alsRijen([{ wijk: 'Oost', aantal: 4 }])!;
    expect(alsRijen('tekst')).toBeUndefined();
    expect(alsRijen([])).toBeUndefined();
    expect(kolommen({ naam: 'x', kolommen: [] }, r).map((k) => k.name)).toEqual(['wijk', 'aantal']);
    const w = teken({ tabel: { naam: 'x', kolommen: [], label: 'Plaatsen' }, rijen: r });
    expect(w.find('nldd-table-row[slot="header"]').findAll('nldd-text-cell').map((c) => c.attributes('text'))).toEqual(['Wijk', 'Aantal']);
    expect(w.findAll('nldd-tag')).toHaveLength(0);
  });
});
