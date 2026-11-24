// The sheet of a request in the timeline: its title anchors the bar, and a
// request without an answer says so in a banner.
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import type { Verzoekregel } from './tijdlijn';
import type { Gram } from './types';
import VerzoekSheet from './VerzoekSheet.vue';

const rij = (antwoord: Record<string, unknown> | null): Verzoekregel => ({
  gram: { id: 'g1' } as Gram,
  tekst: 'Verzoek om een medisch advies',
  regel: '',
  stand: null,
  verzoek: { van: 'gemeente Voorbeeld', aan: 'het adviesbureau', artikel: 'artikel 3', automatisch: false, stappen: [], antwoord },
});

const toon = (r: Verzoekregel) => mount(VerzoekSheet, { props: { rij: r }, global: { stubs: { teleport: true } } });

describe('the request sheet', () => {
  it('anchors the bar to the title in the content', () => {
    const w = toon(rij(null));
    const titel = w.find('nldd-title[heading-level="2"]');
    expect(titel.attributes('text')).toBe('Verzoek om een medisch advies');
    expect(w.find('nldd-top-title-bar').attributes('collapse-anchor')).toBe(titel.attributes('id'));
  });

  it('says in a banner that there is no answer yet', () => {
    const w = toon(rij(null));
    expect(w.find('nldd-banner').attributes('text')).toBe('Er is nog geen antwoord');
  });

  it('shows the answer under an h3', () => {
    const w = toon(rij({ advies: 'ja' }));
    expect(w.find('nldd-banner').exists()).toBe(false);
    const kop = w.find('nldd-title[heading-level="3"]');
    expect(kop.attributes('text')).toBe('Antwoord');
    expect(kop.attributes('size')).toBe('display-5');
  });
});
