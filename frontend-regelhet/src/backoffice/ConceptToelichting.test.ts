// The draft explanation by Vlam under a decision: asked for on a click, marked
// as the draft of a language model, editable, and gone on discarding.
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import ConceptToelichting from './ConceptToelichting.vue';

const concepttoelichting = vi.fn(async (_besluit: string) => ({ concept: 'U krijgt de kaart.', model: 'test-model' }));
vi.mock('./api', () => ({
  backofficeApi: { concepttoelichting: (b: string) => concepttoelichting(b) },
}));

const knop = (w: ReturnType<typeof mount>, tekst: string) => {
  const b = w.findAll('nldd-button').find((x) => x.attributes('text') === tekst);
  if (!b) throw new Error(`no button ${tekst}`);
  return b;
};

describe('ConceptToelichting', () => {
  beforeEach(() => concepttoelichting.mockClear());

  it('asks for nothing until the decider clicks', () => {
    mount(ConceptToelichting, { props: { besluit: 'e' } });
    expect(concepttoelichting).not.toHaveBeenCalled();
  });

  it('shows the draft as the draft of a language model, and discards it', async () => {
    const w = mount(ConceptToelichting, { props: { besluit: 'e' } });
    await knop(w, 'Concepttoelichting laten opstellen').trigger('click');
    await flushPromises();
    expect(concepttoelichting).toHaveBeenCalledWith('e');
    const banner = w.find('nldd-banner');
    expect(banner.attributes('text')).toBe('Concept van een taalmodel');
    expect(banner.attributes('supporting-text')).toContain('test-model');
    expect(w.find('nldd-multi-line-text-field').attributes('value')).toBe('U krijgt de kaart.');

    await knop(w, 'Verwerpen').trigger('click');
    expect(w.find('nldd-multi-line-text-field').exists()).toBe(false);
    expect(w.find('nldd-banner').exists()).toBe(false);
  });

  it('says why there is no draft when Vlam fails', async () => {
    concepttoelichting.mockRejectedValueOnce(new Error('Vlam answered 429'));
    const w = mount(ConceptToelichting, { props: { besluit: 'e' } });
    await knop(w, 'Concepttoelichting laten opstellen').trigger('click');
    await flushPromises();
    const d = w.find('nldd-inline-dialog');
    expect(d.attributes('text')).toBe('Er is geen concept');
    expect(d.attributes('supporting-text')).toContain('429');
  });
});
