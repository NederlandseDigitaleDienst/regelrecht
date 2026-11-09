// The action sheet behind the flag panel.regelrijm: it reads the selected part
// as RegelRijm text, and opens at the part a reading in the Machine pane
// pointed at.
import { describe, it, expect, vi, beforeAll } from 'vitest';
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils';
import { reactive } from 'vue';

vi.mock('../composables/useFeatureFlags', () => ({ useFeatureFlags: () => ({ isEnabled: () => true }) }));
vi.mock('../composables/useLaw', () => ({ fetchLaw: vi.fn(async () => null) }));
const { default: ActionSheet } = await import('./ActionSheet.vue');

const ACTION = { output: 'hoogte', value: { operation: 'MAX', values: [0, { operation: 'MULTIPLY', values: ['$inkomen', '$percentage'] }] } };
const ARTICLE = { number: '2', machine_readable: { execution: { actions: [ACTION] } } };

beforeAll(() => {
  // jsdom does not know nldd-sheet's show() and hide().
  const proto = HTMLElement.prototype as HTMLElement & { show?: () => void; hide?: () => void };
  proto.show ??= () => {};
  proto.hide ??= () => {};
});

const reading = (wrapper: VueWrapper) => wrapper.find('[data-testid="action-sheet-regelrijm"] [data-testid="regelrijm-text"]').text();

describe('ActionSheet with RegelRijm', () => {
  it('reads the whole action when it opens at the top', async () => {
    const wrapper = mount(ActionSheet, { props: { regelrijm: true, action: ACTION, article: ARTICLE } });
    await flushPromises();
    expect(reading(wrapper)).toBe('[hoogte] wordt: de grootste van ( 0; [inkomen] maal [percentage] )');
  });

  it('opens at the part a reading pointed at', async () => {
    const wrapper = mount(ActionSheet, { props: { regelrijm: true, action: ACTION, article: ARTICLE, focusPath: ['value', 'values', 1] } });
    await flushPromises();
    expect(reading(wrapper)).toBe('[inkomen] maal [percentage]');
  });

  it('shows no reading where the view does not ask for one', async () => {
    // The library mounts the sheet without handling navigate; the flag alone
    // must not put a reading there.
    const wrapper = mount(ActionSheet, { props: { action: ACTION, article: ARTICLE } });
    await flushPromises();
    expect(wrapper.find('[data-testid="action-sheet-regelrijm"]').exists()).toBe(false);
  });

  describe('following a name to the rule that computes it', () => {
    const total = () => reactive({ output: 'totaal', value: { operation: 'ADD', values: ['$deel', 1] } });
    const article = (a: object) => ({ number: '2', machine_readable: { execution: { actions: [a, { output: 'deel', value: 5 }] } } });
    const follow = (wrapper: VueWrapper) => wrapper.find('[data-testid^="regelrijm-follow-"]');

    it('opens that rule when the sheet has no unsaved edit', async () => {
      const a = total();
      const wrapper = mount(ActionSheet, { props: { regelrijm: true, action: a, article: article(a) } });
      await flushPromises();
      await follow(wrapper).trigger('click');
      await flushPromises();
      expect(wrapper.emitted('open-output')).toEqual([['deel']]);
    });

    it('stays put and says why when there is an unsaved edit', async () => {
      const a = total();
      const wrapper = mount(ActionSheet, { props: { regelrijm: true, action: a, article: article(a) } });
      await flushPromises();
      a.value.values[1] = 2;
      await flushPromises();
      await follow(wrapper).trigger('click');
      await flushPromises();
      expect(wrapper.emitted('open-output')).toBeUndefined();
      expect(wrapper.emitted('navigate')).toBeUndefined();
      expect(wrapper.html()).toContain('Sla je wijziging eerst op');
    });
  });
});
