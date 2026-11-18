import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import MachineEmptyState from './MachineEmptyState.vue';

// Het knoplabel draagt twee dingen tegelijk die alleen in de tekst zichtbaar
// zijn en die een volgende bewerking dus stilzwijgend kan platslaan:
//
//  1. de scope - de aanvraag kent alleen een wet en levert een voorstel per
//     gewijzigd artikel op, dus het label mag geen artikel-scope suggereren;
//  2. de vorm - gebiedende wijs waar de klik de actie hier uitvoert, heel
//     werkwoord waar hij je eerst ergens heen brengt (login of traject).
//
// Geen enkele andere test raakt deze strings, terwijl de knop in vier panes
// staat (Machine en YAML, in de editor en in de Bibliotheek).

function mountEmpty(needs = '') {
  return mount(MachineEmptyState, {
    props: { canEnrich: true, needs },
  });
}

function enrichLabel(wrapper: ReturnType<typeof mountEmpty>) {
  return wrapper.find('[data-testid="enrich-btn"]').attributes('text');
}

describe('MachineEmptyState', () => {
  it('noemt het regelwerk en niet het artikel waar de knop de actie hier uitvoert', () => {
    const label = enrichLabel(mountEmpty(''));
    expect(label).toBe('Verrijk dit regelwerk');
    expect(label).not.toMatch(/artikel/i);
  });

  it.each(['login', 'traject'])('gebruikt het hele werkwoord waar de knop routeert (%s)', (needs) => {
    const label = enrichLabel(mountEmpty(needs));
    expect(label).toBe('Dit regelwerk verrijken');
    expect(label).not.toMatch(/artikel/i);
  });

  it('zegt in de ondersteunende tekst dat de verrijking de hele wet raakt', () => {
    const supporting = mountEmpty('')
      .find('[data-testid="no-machine-readable"]')
      .attributes('supporting-text');
    expect(supporting).toContain('alle artikelen van deze versie');
  });

  describe('choice of model', () => {
    it('runs the default when the button itself is clicked', async () => {
      const w = mountEmpty('');
      await w.find('[data-testid="enrich-btn"]').trigger('action-click');
      expect(w.emitted('enrich')).toHaveLength(1);
      expect(w.emitted('enrich')?.[0]?.[1]).toBeUndefined();
    });

    it('passes the provider picked from the menu, shown under its own name', async () => {
      const w = mountEmpty('');
      const vlam = w.find('[data-testid="enrich-with-opencode"]');
      expect(vlam.attributes('text')).toBe('VLAM');
      await vlam.trigger('select');
      expect(w.emitted('enrich')).toHaveLength(1);
      expect(w.emitted('enrich')?.[0]?.[1]).toBe('opencode');
    });

    it.each(['login', 'traject'])('offers no menu where the button routes first (%s)', (needs) => {
      const w = mountEmpty(needs);
      expect(w.find('nldd-split-button').exists()).toBe(false);
      expect(w.find('[data-testid="enrich-with-opencode"]').exists()).toBe(false);
    });
  });
});
