import { describe, it, expect, vi } from 'vitest';
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

  describe('with the scope modes on', () => {
    function mountScoped() {
      return mount(MachineEmptyState, { props: { canEnrich: true, needs: '', scoped: true } });
    }

    it('opens the sheet instead of starting a run on the whole law', async () => {
      const w = mountScoped();
      expect(enrichLabel(w)).toBe('Verrijk…');
      await w.find('[data-testid="enrich-btn"]').trigger('action-click');
      expect(w.emitted('enrich')).toBeUndefined();
      expect(w.emitted('enrich-options')).toHaveLength(1);
    });

    it('offers no menu item that starts a whole-law run at once', async () => {
      const w = mountScoped();
      expect(w.find('[data-testid="enrich-with-opencode"]').exists()).toBe(false);
      await w.find('[data-testid="enrich-article"]').trigger('select');
      expect(w.emitted('enrich-article')).toHaveLength(1);
      expect(w.emitted('enrich')).toBeUndefined();
    });

    it('does not promise every article', () => {
      const supporting = mountScoped()
        .find('[data-testid="no-machine-readable"]')
        .attributes('supporting-text');
      expect(supporting).not.toContain('alle artikelen');
    });
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

  describe('stopping a running enrichment', () => {
    function mountBusy(props: Record<string, unknown> = {}) {
      return mount(MachineEmptyState, {
        props: { canEnrich: true, enriching: true, scoped: true, ...props },
      });
    }

    it('offers a stop that asks first, and emits only after confirmation', async () => {
      const w = mountBusy();
      // The custom elements are not registered here: give the dialog the two
      // methods the component calls on it.
      const dialog = w.find('nldd-modal-dialog').element as HTMLElement & { show?: () => void; hide?: () => void };
      dialog.show = vi.fn();
      dialog.hide = vi.fn();
      const stop = w.find('[data-testid="stop-enrich-btn"]');
      expect(stop.exists()).toBe(true);
      await stop.trigger('click');
      expect(dialog.show).toHaveBeenCalled();
      expect(w.emitted('stop-enrich')).toBeUndefined();
      await w.find('[data-testid="confirm-stop-enrich"]').trigger('click');
      expect(w.emitted('stop-enrich')).toHaveLength(1);
    });

    it('offers no stop without the scope modes', () => {
      const w = mountBusy({ scoped: false });
      expect(w.find('[data-testid="enriching"]').exists()).toBe(true);
      expect(w.find('[data-testid="stop-enrich-btn"]').exists()).toBe(false);
    });

    it('says the run is stopping once a stop was asked for', () => {
      const w = mountBusy({ stopping: true });
      expect(w.find('[data-testid="enrich-stopping"]').attributes('text')).toBe('Stoppen…');
      expect(w.find('[data-testid="stop-enrich-btn"]').exists()).toBe(false);
    });
  });
});
