import { mount } from '@vue/test-utils';
import { describe, it, expect } from 'vitest';
import MarkingDetailPanel from './MarkingDetailPanel.vue';
import type { Marking } from '../composables/useMarkings';

// The untranslatable panel it is modelled on has no test, because it only maps
// fields onto rows. This one carries decisions worth pinning: an empty
// `target` means something, and a marking is read beside the words it hangs
// on.
function marking(overrides: Partial<Marking> = {}): Marking {
  return {
    id: '1',
    law_id: 'wet_a',
    law_name: 'Wet A',
    article: '5',
    about: 'de eerstvolgende werkdag',
    resolution: 'operation',
    resolved_by: 'een WORKING_DAY-bewerking',
    target: ['datum_van_betaling'],
    legal_text_excerpt: 'De betaling geschiedt op de eerstvolgende werkdag',
    provider: 'opencode',
    enrich_job_id: 'job-1',
    accepted: false,
    created_at: '2026-09-20T12:00:00Z',
    ...overrides,
  };
}

const global = {
  config: { compilerOptions: { isCustomElement: (tag: string) => tag.startsWith('nldd-') } },
  stubs: { Teleport: true },
};

describe('MarkingDetailPanel', () => {
  it('renders nothing without a row', () => {
    const w = mount(MarkingDetailPanel, { props: { row: null }, global });
    expect(w.html()).not.toContain('Wet A');
  });

  // An empty target is a claim rather than a blank: the article still runs.
  // A dash would leave the reader to guess which of the two it means.
  it('says the article still works when nothing is blocked', () => {
    const w = mount(MarkingDetailPanel, {
      props: { row: marking({ target: [] }), isOpen: true },
      global,
    });
    expect(w.html()).toContain('niets, het artikel blijft werken');
  });

  it('names what is blocked when something is', () => {
    const w = mount(MarkingDetailPanel, {
      props: { row: marking({ target: ['a', 'b'] }), isOpen: true },
      global,
    });
    expect(w.html()).toContain('a, b');
  });

  // The resolution is a closed vocabulary; showing the raw value would make a
  // reader look up what 'model' means.
  it('translates the resolution into what it means', () => {
    const operation = mount(MarkingDetailPanel, {
      props: { row: marking(), isOpen: true },
      global,
    });
    expect(operation.html()).toContain('bewerking ontbreekt');

    const model = mount(MarkingDetailPanel, {
      props: { row: marking({ resolution: 'model' }), isOpen: true },
      global,
    });
    expect(model.html()).toContain('formaat mist een vorm');
  });

  // The schema requires `legal_text_excerpt` because a marking that cannot
  // quote the text it is about is about something else. Reading the two side
  // by side is how someone decides whether the gap is real.
  it('shows the legal text the marking hangs on', () => {
    const w = mount(MarkingDetailPanel, { props: { row: marking(), isOpen: true }, global });
    expect(w.html()).toContain('De wettekst waar het aan hangt');
    expect(w.html()).toContain('De betaling geschiedt op de eerstvolgende werkdag');
  });

  // `resolved_by` is nullable, and a section for a field that is not there
  // would read as an empty answer rather than an absent one.
  it('omits the section for a change that was never named', () => {
    const w = mount(MarkingDetailPanel, {
      props: { row: marking({ resolved_by: null }), isOpen: true },
      global,
    });
    expect(w.html()).not.toContain('Wat het zou oplossen');
    expect(w.html()).toContain('Wat niet uitdrukbaar is');
  });

  it('falls back to the law id when the law has no name', () => {
    const w = mount(MarkingDetailPanel, {
      props: { row: marking({ law_name: null }), isOpen: true },
      global,
    });
    expect(w.html()).toContain('wet_a');
  });
});
