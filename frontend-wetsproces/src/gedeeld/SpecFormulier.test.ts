// The form a screen draws from the law and the UI spec. The nldd-* elements
// are not defined in this test, so it reads the attributes Vue sets on them.
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { aanvraagformulier } from './formulier';
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
    expect(w.findAll('nldd-switch-field').map((e) => e.attributes('label'))).toEqual(['Wil een vergunning']);
    // A yes or no that may stay empty asks ja, nee or nothing.
    const keuze = w.findAll('nldd-form-field').map((e) => e.attributes('label'));
    expect(keuze).toEqual(['Ondertekend', 'Dagtekening']);
    expect(w.find('select').findAll('option').map((o) => o.text())).toEqual(['Niet opgegeven', 'Ja', 'Nee']);
    expect(w.findAll('nldd-date-field')).toHaveLength(1);
    expect(w.html()).not.toContain('Nummer aanvrager');
    expect(w.html()).not.toContain('Geheim');
  });

  it('takes the label and help the UI spec gives a field', () => {
    const w = teken({ dagtekening: { label: 'Datum van uw aanvraag', help: 'De dag waarop u ondertekent.' } });
    const veld = w.findAll('nldd-form-field').find((e) => e.attributes('label') === 'Datum van uw aanvraag');
    expect(veld?.find('nldd-form-field-help-text').text()).toBe('De dag waarop u ondertekent.');
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
    const labels = w.findAll('nldd-form-field, nldd-switch-field').map((e) => e.attributes('label'));
    expect(labels).toEqual(['Dagtekening', 'Ondertekend', 'Wil een vergunning']);
  });

  it('shows a field that is already filled read-only, in its place in its group', () => {
    const { groepen } = aanvraagformulier(vorm, {
      nummer_aanvrager: { group: 'Over u', help: 'Uw nummer.' },
      wil_een_vergunning: { group: 'Over u' },
    }, '999990019');
    const w = mount(SpecFormulier, { props: { velden: groepen.flatMap((g) => g.velden), waarden: {}, volgorde: ['nummer_aanvrager', 'wil_een_vergunning'] } });
    expect(w.findAll('nldd-title').map((t) => t.attributes('text'))).toEqual(['Over u']);
    const eerste = w.findAll('nldd-form-field, nldd-switch-field')[0]!;
    expect(eerste.attributes('label')).toBe('Nummer aanvrager');
    expect(eerste.find('nldd-text-field').attributes('value')).toBe('999990019');
    expect(eerste.find('nldd-text-field').attributes()).toHaveProperty('readonly');
    expect(eerste.find('nldd-form-field-help-text').text()).toBe('Uw nummer. Uit de inlog van de aanvrager.');
    // A field filled already is never sent up as a change.
    expect(w.emitted('zet')).toBeUndefined();
  });

  it('puts the help of a switch under it, in a field without a second label', () => {
    const w = teken({ wil_een_vergunning: { help: 'Zet dit aan als u er een wilt.' } });
    const schakelaar = w.find('nldd-switch-field');
    expect(schakelaar.attributes('label')).toBe('Wil een vergunning');
    // A rendered switch always has a parent: the test mounts it inside the form.
    const veld = schakelaar.element.parentElement!;
    expect(veld.tagName.toLowerCase()).toBe('nldd-form-field');
    expect(veld.getAttribute('label')).toBeNull();
    expect(veld.querySelector('nldd-form-field-help-text')?.textContent).toBe('Zet dit aan als u er een wilt.');
    // Without help a switch stands on its own, as before.
    expect(teken().find('nldd-switch-field').element.parentElement?.tagName.toLowerCase()).not.toBe('nldd-form-field');
  });

  it('sends a change of a field up by its name', async () => {
    const w = teken();
    await w.find('select').setValue('ja');
    expect(w.emitted('zet')).toEqual([['ondertekend', true]]);
  });
});
