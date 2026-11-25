// What a record holds, as a screen shows it: in the order of the UI spec,
// under its groups, with its label and help. The nldd-* elements are not
// defined in this test, so it reads the attributes Vue sets on them.
import { mount } from '@vue/test-utils';
import type { DOMWrapper } from '@vue/test-utils';
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

/** The name of a row: the label cell's text, without the info button after it. */
const labelVan = (r: DOMWrapper<Element>) => {
  const cel = r.find('nldd-text-cell');
  return cel.attributes('text') ?? cel.find('p').element.firstChild?.textContent?.trim();
};
const rijenVan = (w: ReturnType<typeof teken>) => w.findAll('nldd-list').map((l) => l.findAll('nldd-list-item').map(labelVan));
/** The headings of the groups: the texts outside the popovers. */
const koppen = (w: ReturnType<typeof teken>) =>
  w.findAll('nldd-text').filter((t) => !t.element.closest('nldd-popover')).map((t) => t.text());

describe('Gegevens', () => {
  it('shows the fields in the order of the UI spec, under the first place each group stands', () => {
    const w = teken();
    expect(koppen(w)).toEqual(['Uw gegevens', 'Over deze aanvraag']);
    const lijsten = rijenVan(w);
    // The address joins its group although the date stands between; a field
    // the UI spec does not name follows, and an empty one stays out.
    expect(lijsten).toEqual([['Naam', 'Adres'], ['Dagtekening'], ['Overig']]);
  });

  it('puts the origin and the help of the UI spec in a popover after the label, and the value on the right', () => {
    const w = teken({ herkomst: { adres: 'Uit de inlog van de aanvrager.' } });
    const rij = w.findAll('nldd-list-item').find((r) => labelVan(r) === 'Adres')!;
    const [label, waarde] = rij.findAll('nldd-text-cell');
    expect(label!.attributes('supporting-text')).toBeUndefined();
    const knop = label!.find('nldd-icon-button');
    expect(knop.attributes('size')).toBe('xs');
    expect(knop.attributes('icon')).toBe('info');
    expect(knop.attributes('appearance')).toBe('neutral-base');
    const popover = w.find(`nldd-popover[id="${knop.attributes('popovertarget')}"]`);
    expect(popover.findAll('nldd-text').map((t) => t.text())).toEqual(['Uit de inlog van de aanvrager.', 'Zoals in de basisregistratie.']);
    expect([knop.attributes('text'), popover.attributes('role')]).toEqual(['Over adres', 'region']);
    expect(waarde!.attributes('text')).toBe('Voorbeeldlaan 2');
    // An abbreviation keeps its capitals in the name of the button.
    const bsn = teken({ waarden: { bsn: '999990019' }, teksten: { bsn: { label: 'BSN', help: 'Uit de inlog.' } }, volgorde: ['bsn'] });
    expect(bsn.find('nldd-icon-button').attributes('text')).toBe('Over BSN');
    // A field with neither has no button.
    const naam = w.findAll('nldd-list-item').find((r) => labelVan(r) === 'Naam')!;
    expect(naam.find('nldd-icon-button').exists()).toBe(false);
    expect(waarde!.attributes('horizontal-alignment')).toBe('right');
    // The value is as wide as its words, beside its check mark or cross.
    expect([label!.attributes('width'), waarde!.attributes('width')]).toEqual([undefined, 'fit-content']);
    // A row that is no field, such as the article, turns that round: its value can run long.
    const artikel = teken({ bij: [{ naam: 'grondslag', label: 'Op grond van', waarde: 'Wet artikel 1' }] })
      .findAll('nldd-list-item')
      .find((r) => labelVan(r) === 'Op grond van')!
      .findAll('nldd-text-cell');
    expect(artikel.map((c) => c.attributes('width'))).toEqual(['fit-content', undefined]);
  });

  it('heads its groups at the level it is given, at one size', () => {
    const w = teken({ kopniveau: 3 });
    const titels = w.findAll('nldd-title');
    expect(titels.map((t) => [t.attributes('text'), t.attributes('heading-level'), t.attributes('size')])).toEqual([
      ['Uw gegevens', '3', 'display-5'],
      ['Over deze aanvraag', '3', 'display-5'],
    ]);
    expect(koppen(w)).toEqual([]);
  });

  it('keeps the order it gets without a UI spec, under no heading', () => {
    const w = teken({ teksten: undefined, volgorde: undefined });
    expect(w.findAll('nldd-text')).toHaveLength(0);
    expect(w.findAll('nldd-list-item').map(labelVan)).toEqual(['Adres', 'Datum', 'Naam', 'Overig']);
  });

  it('leads with the group that says what the record is about, and adds what is no field of the record to it', () => {
    const bij = [{ naam: 'grondslag', label: 'Op grond van', waarde: 'Wet artikel 1' }];
    // The group of the first field the UI spec marks as a summary, not the personal details before it.
    const metSamenvatting = { ...teksten, datum: { ...teksten.datum, record: { summary: true } } };
    // That group leads, without a heading: the screen is about that record.
    const w = teken({ bij, teksten: metSamenvatting });
    expect(rijenVan(w)).toEqual([['Dagtekening', 'Op grond van'], ['Naam', 'Adres'], ['Overig']]);
    expect(koppen(w)).toEqual(['Uw gegevens']);
    // Without a summary, a group of its own before the others, not among the personal details.
    expect(rijenVan(teken({ bij }))).toEqual([['Op grond van'], ['Naam', 'Adres'], ['Dagtekening'], ['Overig']]);
    expect(koppen(teken({ bij }))).toEqual(['Uw gegevens', 'Over deze aanvraag']);
  });

  it('shows what the record is about apart from the rest, the summary first and the article after it, with rows of the screen in front', () => {
    const bij = [{ naam: 'grondslag', label: 'Op grond van', waarde: 'Wet artikel 1' }];
    const metSamenvatting = { ...teksten, naam: { ...teksten.naam, group: 'Over deze aanvraag' }, datum: { ...teksten.datum, record: { summary: true } } };
    const over = mount(Gegevens, {
      props: {
        waarden: { adres: 'Voorbeeldlaan 2', datum: '2026-10-13', naam: 'A. de Vries', overig: true },
        teksten: metSamenvatting,
        volgorde: ['naam', 'datum', 'adres'],
        label: 'Gegevens',
        bij,
        deel: 'over' as const,
      },
      slots: { voor: '<nldd-list-item><nldd-text-cell text="Status"></nldd-text-cell></nldd-list-item>' },
    });
    expect(rijenVan(over)).toEqual([['Status', 'Dagtekening', 'Op grond van', 'Naam']]);
    expect(koppen(over)).toEqual([]);
    // The rest has no article to add, and still leaves out what the record is about.
    const rest = teken({ teksten: metSamenvatting, deel: 'rest' });
    expect(rijenVan(rest)).toEqual([['Adres'], ['Overig']]);
    expect(koppen(rest)).toEqual(['Uw gegevens']);
    // Nothing left over says nothing.
    expect(mount(Gegevens, { props: { waarden: {}, label: 'Gegevens', deel: 'rest' as const } }).text()).toBe('');
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
    // Without words under `record`, the form's.
    const opFormulier = teken({ waarden: { o: false }, teksten: { o: { when_true: 'Onderzoek achterwege', when_false: 'Onderzoek nodig' } }, volgorde: ['o'] });
    expect(opFormulier.findAll('nldd-text-cell').at(-1)!.attributes('text')).toBe('Onderzoek nodig');
    // A check mark or a cross stands before the words.
    expect(w.findAll('nldd-list-item').map((r) => r.find('nldd-icon-cell').attributes('icon'))).toEqual(['checked', 'dismiss', 'checked']);
    expect(teken().findAll('nldd-list-item').filter((r) => r.find('nldd-icon-cell').exists()).map(labelVan)).toEqual(['Overig']);
  });

  it('shows a list of rows as a table under the list, where the UI spec gives the field a table', () => {
    const lijst = {
      waarden: { naam: 'A. de Vries', zetels: [{ gebied: 'Wilgendaal', aantal_zetels: 3 }] },
      teksten: { naam: { label: 'Naam' }, zetels: { label: 'Zetels per orgaan' } },
      volgorde: ['naam', 'zetels'],
    };
    // Without a table for the field, the value stays a row.
    expect(rijenVan(teken(lijst))).toEqual([['Naam', 'Zetels per orgaan']]);
    expect(teken(lijst).find('nldd-table').exists()).toBe(false);
    const w = teken({ ...lijst, tabellen: [{ naam: 'UI1.zetels', veld: 'zetels', kolommen: [] }] });
    expect(rijenVan(w)).toEqual([['Naam']]);
    const tabel = w.find('nldd-table');
    expect(tabel.attributes('accessible-label')).toBe('Zetels per orgaan');
    expect(tabel.attributes('columns')).toBe('minmax(max-content, 1fr) minmax(max-content, 1fr)');
    expect(tabel.findAll('nldd-text-cell').map((c) => c.attributes('text'))).toEqual(['Gebied', 'Aantal zetels', 'Wilgendaal', '3']);
  });
});
