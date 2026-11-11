// How the action sheet closes. Escape belongs to nldd-sheet: it is a modal
// <dialog> that turns Escape into its own close, with the closing animation.
// A second listener on the document closed the sheet as well when Escape was
// meant for a menu or popover inside it.
import { describe, it, expect, beforeAll, afterEach, vi } from 'vitest';
import { mount, flushPromises, enableAutoUnmount } from '@vue/test-utils';

vi.mock('../composables/useFeatureFlags', () => ({ useFeatureFlags: () => ({ isEnabled: () => false }) }));
const { default: ActionSheet } = await import('./ActionSheet.vue');

const ACTION = { output: 'hoogte', value: 5 };
const ARTICLE = { number: '2', machine_readable: { execution: { actions: [ACTION] } } };

beforeAll(() => {
  // happy-dom does not know nldd-sheet's show() and hide().
  const proto = HTMLElement.prototype as HTMLElement & { show?: () => void; hide?: () => void };
  proto.show ??= () => {};
  proto.hide ??= () => {};
});

enableAutoUnmount(afterEach);

describe('ActionSheet closing', () => {
  it('leaves Escape to the sheet', async () => {
    const wrapper = mount(ActionSheet, { props: { action: ACTION, article: ARTICLE }, attachTo: document.body });
    await flushPromises();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(wrapper.emitted('close')).toBeUndefined();
  });

  it('closes when the sheet does', async () => {
    const wrapper = mount(ActionSheet, { props: { action: ACTION, article: ARTICLE } });
    await flushPromises();
    wrapper.find('nldd-sheet').element.dispatchEvent(new CustomEvent('close'));
    expect(wrapper.emitted('close')).toHaveLength(1);
  });
});
