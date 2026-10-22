import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';

const apiFetch = vi.fn();
vi.mock('../lib/apiFetch', () => ({ apiFetch: (...args: unknown[]) => apiFetch(...args) }));

import LawOverviewPane from './LawOverviewPane.vue';
import { enrichedOnly } from '../composables/useLawOverview';

function response(list: unknown[], total: number) {
  return { json: async () => list, headers: { get: () => String(total) } };
}

async function settle() {
  for (let i = 0; i < 5; i++) await nextTick();
}

beforeEach(() => {
  apiFetch.mockReset();
  enrichedOnly.value = false;
});

describe('LawOverviewPane', () => {
  it('lists the laws and marks the ones with a machine version', async () => {
    apiFetch.mockResolvedValue(response([
      { law_id: 'wet_a', display_name: 'Wet A', source_name: 'Voorbeelden', enriched: true },
      { law_id: 'wet_b', display_name: 'Wet B', source_name: 'Centraal', enriched: false },
    ], 2));
    const wrapper = mount(LawOverviewPane, { props: { backText: 'Corpus juris' } });
    await settle();

    const cells = wrapper.findAll('[data-testid="overview-list"] nldd-text-cell');
    expect(cells.map((c) => c.attributes('text'))).toEqual(['Wet A', 'Wet B']);
    expect(cells.map((c) => c.attributes('supporting-text'))).toEqual([
      'Voorbeelden · machineversie',
      'Centraal',
    ]);
  });

  it('opens a law by emitting its id', async () => {
    apiFetch.mockResolvedValue(response([{ law_id: 'wet_a' }], 1));
    const wrapper = mount(LawOverviewPane, { props: { backText: 'Corpus juris' } });
    await settle();

    await wrapper.find('[data-law-id="wet_a"]').trigger('click');
    expect(wrapper.emitted('select')).toEqual([['wet_a']]);
  });

  it('turns the shared enriched filter on from its checkbox', async () => {
    apiFetch.mockResolvedValue(response([], 0));
    const wrapper = mount(LawOverviewPane, { props: { backText: 'Corpus juris' } });
    await settle();

    const box = wrapper.find('[data-testid="overview-enriched-filter"]');
    box.element.dispatchEvent(new CustomEvent('change', { detail: { checked: true } }));
    await settle();

    expect(enrichedOnly.value).toBe(true);
    expect(String(apiFetch.mock.calls.at(-1)?.[0])).toContain('enriched=true');
    // With nothing enriched yet the pane says so, rather than showing an empty list.
    expect(wrapper.find('nldd-inline-dialog').attributes('text')).toBe('Nog geen verrijkte regelwerken');
  });
});
