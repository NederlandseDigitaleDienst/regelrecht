// What a record holds, as a screen shows it: in the order of the UI spec,
// under its groups, with its label and help. The nldd-* elements are not
// defined in this test, so it reads the attributes Vue sets on them.
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import Gegevens from './Gegevens.vue';

const teksten = {
  adres: { label: 'Adres', group: 'Uw gegevens', help: 'Zoals in de basisregistratie.' },
  naam: { label: 'Naam', group: 'Uw gegevens' },
  datum: { label: 'Dagtekening', group: 'Over deze aanvraag' },
};

function teken(props: Record<string, unknown> = {}) {
  return mount(Gegevens, {
    props: {
      // serde_json gives the fields in alphabetical order.
      waarden: { adres: 'Voorbeeldlaan 2', datum: '2026-10-13', leeg: null, naam: 'A. de Vries', overig: true },
      teksten,
      volgorde: ['naam', 'datum', 'adres'],
      label: 'Gegevens',
      ...props,
    },
  });
}

describe('Gegevens', () => {
  it('shows the fields in the order of the UI spec, under the first place each group stands', () => {
    const w = teken();
    expect(w.findAll('nldd-text').map((t) => t.text())).toEqual(['Uw gegevens', 'Over deze aanvraag']);
    const lijsten = w.findAll('nldd-list').map((l) => l.findAll('nldd-text-cell').map((c) => c.attributes('text')));
    // The address joins its group although the date stands between; a field
    // the UI spec does not name follows, and an empty one stays out.
    expect(lijsten).toEqual([['Naam', 'Adres'], ['Dagtekening'], ['Overig']]);
  });

  it('puts the value, its origin and the help of the UI spec under the label', () => {
    const w = teken({ herkomst: { adres: 'Uit de inlog van de aanvrager.' } });
    const adres = w.findAll('nldd-text-cell').find((c) => c.attributes('text') === 'Adres');
    expect(adres?.attributes('supporting-text')).toBe(
      'Voorbeeldlaan 2 · Uit de inlog van de aanvrager. · Zoals in de basisregistratie.',
    );
  });

  it('keeps the order it gets without a UI spec, under no heading', () => {
    const w = teken({ teksten: undefined, volgorde: undefined });
    expect(w.findAll('nldd-text')).toHaveLength(0);
    expect(w.findAll('nldd-text-cell').map((c) => c.attributes('text'))).toEqual(['Adres', 'Datum', 'Naam', 'Overig']);
  });
});
