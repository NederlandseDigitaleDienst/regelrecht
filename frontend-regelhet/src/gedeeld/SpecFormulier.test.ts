// The form a screen draws from the law and the UI spec. The nldd-* elements
// are not defined in this test, so it reads the attributes Vue sets on them.
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { aanvraagformulier } from './formulier';
import { artikelNaam } from './keten';
import SpecFormulier from './SpecFormulier.vue';
import type { Indieningsvorm } from './types';

const vorm: Indieningsvorm = {
  soort: 'AANVRAAG',
  basis: 'voorbeeldwet#3',
  besluit: 'voorbeeldwet#3',
  cel: 'voorbeeldcel',
  velden: [
    { naam: 'nummer_aanvrager', type: 'string', mag_leeg: false, basis: 'voorbeeldwet#3' },
    { naam: 'wil_een_vergunning', type: 'boolean', mag_leeg: false, basis: 'voorbeeldwet#3' },
    { naam: 'ondertekend', type: 'boolean', mag_leeg: true, basis: 'algemene_wet#4:2' },
    { naam: 'geheim', type: 'string', mag_leeg: true, basis: 'voorbeeldwet#3' },
    { naam: 'dagtekening', type: 'date', mag_leeg: true, basis: 'algemene_wet#4:2' },
  ],
  systeem: { nummer_aanvrager: {} },
  verborgen: ['geheim'],
};

function teken(teksten = {}) {
  const { invullen } = aanvraagformulier(vorm, teksten, '999990019');
  return mount(SpecFormulier, { props: { velden: invullen, waarden: {} } });
}

describe('SpecFormulier', () => {
  it('draws a field per question of the law, by its type, without what the system fills or the spec hides', () => {
    const w = teken();
    expect(w.findAll('nldd-checkbox-field').map((e) => e.attributes('label'))).toEqual(['Wil een vergunning']);
    // A yes or no that may stay empty asks ja, nee or nothing.
    const keuze = w.findAll('nldd-form-field').map((e) => e.attributes('label')).filter(Boolean);
    expect(keuze).toEqual(['Ondertekend', 'Dagtekening']);
    // Three choices stand side by side, not in a dropdown.
    expect(w.find('nldd-dropdown').exists()).toBe(false);
    expect(w.findAll('nldd-radio-button-group nldd-radio-button-field').map((o) => o.attributes('label'))).toEqual(['Niet opgegeven', 'Ja', 'Nee']);
    expect(w.find('nldd-radio-button-field[checked]').attributes('value')).toBe('leeg');
    expect(w.findAll('nldd-date-field')).toHaveLength(1);
    expect(w.html()).not.toContain('Nummer aanvrager');
    expect(w.html()).not.toContain('Geheim');
  });

  it('names the answers of a judgement as the UI spec words them, and else yes and no', () => {
    const { invullen } = aanvraagformulier(vorm, { ondertekend: { when_true: 'Ondertekend', when_false: 'Niet ondertekend' } }, '999990019');
    const w = mount(SpecFormulier, { props: { velden: invullen, waarden: {}, oordeel: true } });
    const keuzes = (label: string) =>
      w
        .findAll('nldd-form-field')
        .find((f) => f.attributes('label') === label)!
        .findAll('nldd-radio-button-field')
        .map((o) => o.attributes('label'));
    expect(keuzes('Ondertekend')).toEqual(['Niet beoordeeld', 'Ondertekend', 'Niet ondertekend']);
    expect(keuzes('Wil een vergunning')).toEqual(['Niet beoordeeld', 'Ja', 'Nee']);
  });

  it('takes the label and help the UI spec gives a field', () => {
    const w = teken({ dagtekening: { label: 'Datum van uw aanvraag', help: 'De dag waarop u ondertekent.' } });
    const veld = w.findAll('nldd-form-field').find((e) => e.attributes('label') === 'Datum van uw aanvraag');
    expect(veld?.find('nldd-form-field-help-text').text()).toBe(
      `De dag waarop u ondertekent. Volgens ${artikelNaam('algemene_wet#4:2')}.`,
    );
  });

  it('orders and groups the fields as the UI spec says, each group once', () => {
    const { invullen } = aanvraagformulier(vorm, {
      ondertekend: { group: 'Over deze aanvraag' },
      dagtekening: { group: 'Over deze aanvraag' },
      wil_een_vergunning: { group: 'Uw aanvraag' },
    }, null);
    const w = mount(SpecFormulier, {
      props: { velden: invullen, waarden: {}, volgorde: ['dagtekening', 'wil_een_vergunning', 'ondertekend'] },
    });
    expect(w.findAll('nldd-title').map((t) => t.attributes('text'))).toEqual(['Over deze aanvraag', 'Uw aanvraag']);
    const labels = w.findAll('nldd-form-field, nldd-checkbox-field').map((e) => e.attributes('label')).filter(Boolean);
    expect(labels).toEqual(['Dagtekening', 'Ondertekend', 'Wil een vergunning']);
  });

  it('shows what is already filled apart, under the questions, as a list of what is known, not as inputs', () => {
    const { groepen } = aanvraagformulier(vorm, {
      nummer_aanvrager: { group: 'Over u', help: 'Uw nummer.' },
      wil_een_vergunning: { group: 'Over u' },
    }, '999990019');
    const w = mount(SpecFormulier, { props: { velden: groepen.flatMap((g) => g.velden), waarden: {}, volgorde: ['nummer_aanvrager', 'wil_een_vergunning'] } });
    // The question keeps its group; what is known gets a heading of its own.
    expect(w.findAll('nldd-title').map((t) => t.attributes('text'))).toEqual(['Over u', 'Wat al bekend is']);
    // The list stands after the questions, although the UI spec names the field first.
    const lijst = w.find('nldd-list');
    expect(lijst.element.compareDocumentPosition(w.find('nldd-checkbox-field').element) & Node.DOCUMENT_POSITION_PRECEDING).toBeTruthy();
    expect(lijst.attributes('accessible-label')).toBe('Wat al bekend is');
    const [naam, waarde] = lijst.findAll('nldd-text-cell');
    expect(naam!.attributes('text')).toBe('Nummer aanvrager');
    expect(naam!.attributes('supporting-text')).toBe(
      `Uw nummer. Volgens ${artikelNaam('voorbeeldwet#3')}. Uit de inlog van de aanvrager.`,
    );
    // The value itself stands on the right of its row.
    expect(waarde!.attributes('text')).toBe('999990019');
    expect(waarde!.attributes('horizontal-alignment')).toBe('right');
    expect(w.find('nldd-text-field').exists()).toBe(false);
    // Without a caption the list follows its heading; with one, the caption stands between.
    expect(w.find('nldd-container nldd-text').exists()).toBe(false);
    const metKop = mount(SpecFormulier, {
      props: { velden: groepen.flatMap((g) => g.velden), waarden: {}, bekendKop: 'Al bekend' },
    });
    expect(metKop.html()).toContain('<p>Al bekend</p>');
    // A field filled already is never sent up as a change.
    expect(w.emitted('zet')).toBeUndefined();
  });

  it('puts the help of a checkbox and the article under it, in a field without a second label', () => {
    const w = teken({ wil_een_vergunning: { help: 'Zet dit aan als u er een wilt.' } });
    const vinkje = w.find('nldd-checkbox-field');
    expect(vinkje.attributes('label')).toBe('Wil een vergunning');
    // A rendered checkbox always has a parent: the test mounts it inside the form.
    const veld = vinkje.element.parentElement!;
    expect(veld.tagName.toLowerCase()).toBe('nldd-form-field');
    expect(veld.getAttribute('label')).toBeNull();
    expect(veld.querySelector('nldd-form-field-help-text')?.textContent).toBe(
      `Zet dit aan als u er een wilt. Volgens ${artikelNaam('voorbeeldwet#3')}.`,
    );
  });

  it('makes one question of a heading whose every field is a bare checkbox, with the article under the group', () => {
    const beide: Indieningsvorm = {
      ...vorm,
      velden: [
        { naam: 'als_bestuurder', type: 'boolean', mag_leeg: false, basis: 'voorbeeldwet#3' },
        { naam: 'als_passagier', type: 'boolean', mag_leeg: false, basis: 'voorbeeldwet#3' },
      ],
      systeem: {},
      verborgen: [],
    };
    const { invullen } = aanvraagformulier(beide, {
      als_bestuurder: { group: 'Uw aanvraag' },
      als_passagier: { group: 'Uw aanvraag' },
    }, null);
    const w = mount(SpecFormulier, { props: { velden: invullen, waarden: {} } });
    const groep = w.find('nldd-checkbox-group');
    expect(groep.exists()).toBe(true);
    expect(groep.findAll('nldd-checkbox-field')).toHaveLength(2);
    // The heading becomes the caption of the field around the group, and the
    // article stands under the group instead of under each box.
    const veld = groep.element.parentElement!;
    expect(veld.tagName.toLowerCase()).toBe('nldd-form-field');
    expect(veld.getAttribute('label')).toBe('Uw aanvraag');
    expect(veld.querySelector('nldd-form-field-help-text')?.textContent).toBe(`Volgens ${artikelNaam('voorbeeldwet#3')}.`);
    expect(w.findAll('nldd-title')).toHaveLength(0);
  });

  it('leaves the article out where a screen of its own holds it', () => {
    const { invullen } = aanvraagformulier(vorm, { dagtekening: { help: 'De dag waarop u ondertekent.' } }, null);
    const w = mount(SpecFormulier, { props: { velden: invullen, waarden: {}, artikelen: false } });
    const veld = w.findAll('nldd-form-field').find((e) => e.attributes('label') === 'Dagtekening');
    // The help the UI spec gives stays; only the article goes.
    expect(veld?.find('nldd-form-field-help-text').text()).toBe('De dag waarop u ondertekent.');
    expect(w.html()).not.toContain('Volgens');
  });

  it('drops the optional badge of a field the UI spec requires, and has the browser refuse it empty', () => {
    const { invullen } = aanvraagformulier(vorm, {}, null, [], {}, ['dagtekening']);
    const w = mount(SpecFormulier, { props: { velden: invullen, waarden: {} } });
    const veld = w.findAll('nldd-form-field').find((e) => e.attributes('label') === 'Dagtekening')!;
    expect(veld.attributes()).not.toHaveProperty('optional');
    expect(veld.find('nldd-date-field').attributes()).toHaveProperty('required');
    // The list says what is wrong once the form refuses it; it has no rule of its own.
    expect(veld.find('nldd-validation-list').exists()).toBe(true);
    // A field the UI spec leaves alone keeps what the law says of it.
    const los = teken().findAll('nldd-form-field').find((e) => e.attributes('label') === 'Dagtekening')!;
    expect(los.attributes()).toHaveProperty('optional');
    expect(los.find('nldd-validation-list').exists()).toBe(false);
  });

  it('makes a text field a box of the lines the UI spec gives it, and sends what is typed', async () => {
    const metToelichting = { ...vorm, velden: [...vorm.velden, { naam: 'toelichting', type: 'string', mag_leeg: true, basis: 'voorbeeldwet#3' }] };
    const { invullen } = aanvraagformulier(metToelichting, { toelichting: { lines: 4 } }, '999990019');
    const w = mount(SpecFormulier, { props: { velden: invullen, waarden: {} } });
    const vak = w.find('nldd-multi-line-text-field');
    expect(vak.attributes('rows')).toBe('4');
    expect(w.findAll('nldd-multi-line-text-field')).toHaveLength(1);
    await vak.trigger('input', { detail: { value: 'Twee zinnen. Of meer.' } });
    expect(w.emitted('zet')).toEqual([['toelichting', 'Twee zinnen. Of meer.']]);
  });

  it('sends a change of a field up by its name', async () => {
    const w = teken();
    await w.find('nldd-radio-button-field[value="ja"]').trigger('change', { detail: { checked: true, value: 'ja' } });
    expect(w.emitted('zet')).toEqual([['ondertekend', true]]);
  });
});
