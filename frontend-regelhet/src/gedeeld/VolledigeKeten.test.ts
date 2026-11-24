// The full chain, folded under one row, with the shared steps: an article by
// the name the case gives its regelwerk, a field by the label of the UI
// spec, and a tag for what filled a term in.
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import type { Casus } from './types';
import VolledigeKeten from './VolledigeKeten.vue';

const casus: Casus = {
  id: 'voorbeeld',
  cel: 'gemeente',
  cel_naam: 'gemeente Voorbeeld',
  artikel: 'besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49',
  besluit: 'besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49',
  andere_cellen: [],
  indieningen: [],
  regelwerken: [
    { id: 'besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw', naam: null, map: 'regulation/nl/amvb/babw' },
    { id: 'beleid_voorbeeld', naam: 'Beleid Voorbeeld', map: 'regulation/nl/uitvoeringsbeleid/beleid_voorbeeld' },
  ],
  vandaag: '2026-10-13',
};
const keten = [
  {
    soort: 'artikel',
    wet: 'besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw',
    artikel: '49',
    stappen: [
      {
        soort: 'begrip',
        naam: 'loopbeperking',
        waarde: true,
        ingevuld_door: { regulation: { law_id: 'beleid_voorbeeld', article: '3' } },
        stappen: [],
      },
    ],
  },
];

describe('the full chain', () => {
  it('folds away under one row, with readable names and what filled each term in', async () => {
    const w = mount(VolledigeKeten, {
      props: { keten, casus, teksten: { loopbeperking: { label: 'Langdurige loopbeperking' } } },
    });
    const kop = w.find('nldd-list-item');
    expect(kop.attributes('expanded')).toBe('false');
    await kop.trigger('click');
    expect(kop.attributes('expanded')).toBe('true');
    const regels = w.findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(regels).toContain('BABW artikel 49 wordt uitgevoerd');
    expect(regels).toContain('Langdurige loopbeperking: ja');
    expect(w.findAll('nldd-tag').map((t) => t.attributes('text'))).toEqual(['wet', 'beleid']);
  });

  it('shows nothing for a decision without a chain', () => {
    const w = mount(VolledigeKeten, { props: { keten: [], casus } });
    expect(w.find('nldd-list').exists()).toBe(false);
  });
});
