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
    const lijsten = w.findAll('nldd-list').map((l) => l.findAll('nldd-list-item').map((r) => r.find('nldd-text-cell').attributes('text')));
    // The address joins its group although the date stands between; a field
    // the UI spec does not name follows, and an empty one stays out.
    expect(lijsten).toEqual([['Naam', 'Adres'], ['Dagtekening'], ['Overig']]);
  });

  it('puts the origin and the help of the UI spec under the label, and the value on the right', () => {
    const w = teken({ herkomst: { adres: 'Uit de inlog van de aanvrager.' } });
    const rij = w.findAll('nldd-list-item').find((r) => r.find('nldd-text-cell').attributes('text') === 'Adres')!;
    const [label, waarde] = rij.findAll('nldd-text-cell');
    expect(label!.attributes('supporting-text')).toBe('Uit de inlog van de aanvrager. · Zoals in de basisregistratie.');
    expect(waarde!.attributes('text')).toBe('Voorbeeldlaan 2');
    expect(waarde!.attributes('horizontal-alignment')).toBe('right');
  });

  it('keeps the order it gets without a UI spec, under no heading', () => {
    const w = teken({ teksten: undefined, volgorde: undefined });
    expect(w.findAll('nldd-text')).toHaveLength(0);
    expect(w.findAll('nldd-list-item').map((r) => r.find('nldd-text-cell').attributes('text'))).toEqual(['Adres', 'Datum', 'Naam', 'Overig']);
  });

  it('adds what is no field of the record at the end of its last group', () => {
    const w = teken({ bij: [{ naam: 'grondslag', label: 'Op grond van', waarde: 'Wet artikel 1' }] });
    const laatste = w.findAll('nldd-list').at(-1)!;
    const rijen = laatste.findAll('nldd-list-item').map((r) => r.findAll('nldd-text-cell').map((c) => c.attributes('text')));
    expect(rijen).toEqual([['Overig', 'Ja'], ['Op grond van', 'Wet artikel 1']]);
  });

  it('names a yes or no in the words the UI spec gives it on a record', () => {
    const w = teken({
      waarden: { kaart: true, ander: false, open: true },
      teksten: {
        kaart: { record: { when_true: 'Aangevraagd', when_false: 'Niet aangevraagd' } },
        ander: { record: { when_true: 'Aangevraagd', when_false: 'Niet aangevraagd' } },
      },
      volgorde: ['kaart', 'ander', 'open'],
    });
    const waarden = w.findAll('nldd-list-item').map((r) => r.findAll('nldd-text-cell').at(-1)!.attributes('text'));
    // A field without such words keeps the plain answer, with a capital as a value on its own.
    expect(waarden).toEqual(['Aangevraagd', 'Niet aangevraagd', 'Ja']);
  });
});
