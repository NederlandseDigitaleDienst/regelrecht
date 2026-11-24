import { describe, it, expect, vi, beforeEach } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import HotspotsSidebarBlock from './HotspotsSidebarBlock.vue';
import type { Hotspots } from '../lib/enrichChain';

const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch', async (importOriginal) => ({
  ...(await importOriginal()),
  apiFetchJson: (...a: unknown[]) => apiFetchJson(...a),
}));

const ANSWER: Hotspots = {
  hotspots: [
    {
      law_id: 'awir',
      bwb_id: 'BWBA',
      article: '8',
      score: 11,
      dependants: 4,
      reasons: [
        { signal: 'M', points: 3, detail: 'Geen machine_readable', refs: [] },
        { signal: 'B', points: 8, detail: '4 artikelen elders lezen dit', refs: ['zorgtoeslag#2'] },
      ],
    },
  ],
  per_article: { 'awir#8': { score: 11, count: 4 } },
  partial: false,
};

describe('HotspotsSidebarBlock', () => {
  beforeEach(() => {
    apiFetchJson.mockReset();
    apiFetchJson.mockResolvedValue(ANSWER);
  });

  it('lists the top hotspots of the traject with their first reason and dependants', async () => {
    const wrapper = mount(HotspotsSidebarBlock, { props: { trajectRef: 'traject-abcd1234', enabled: true } });
    await flushPromises();
    expect(apiFetchJson).toHaveBeenCalledWith('/api/trajects/traject-abcd1234/hotspots?limit=5');
    const row = wrapper.find('[data-hotspot="awir#8"]');
    expect(row.find('nldd-text-cell').attributes('supporting-text')).toBe('Geen machine_readable (+3)');
    expect(row.find('nldd-badge').attributes('number')).toBe('4');
    await row.trigger('click');
    expect(wrapper.emitted('open')).toEqual([['awir', '8']]);
  });

  it('asks nothing and shows nothing with the flag off', async () => {
    const wrapper = mount(HotspotsSidebarBlock, { props: { trajectRef: 'traject-abcd1234', enabled: false } });
    await flushPromises();
    expect(apiFetchJson).not.toHaveBeenCalled();
    expect(wrapper.find('nldd-list').exists()).toBe(false);
  });

  it('folds away', async () => {
    const wrapper = mount(HotspotsSidebarBlock, { props: { trajectRef: 'traject-abcd1234', enabled: true } });
    await flushPromises();
    await wrapper.find('[data-testid="hotspots-toggle"]').trigger('click');
    expect(wrapper.find('nldd-list').exists()).toBe(false);
  });
});
