// How the action sheet closes. Escape belongs to nldd-sheet: it is a modal
// <dialog> that turns Escape into its own close, with the closing animation.
// A second listener on the document closed the sheet as well when Escape was
// meant for a menu or popover inside it.
import { describe, it, expect, beforeAll, afterEach } from 'vitest';
import { mount, flushPromises, enableAutoUnmount } from '@vue/test-utils';

import ActionSheet from './ActionSheet.vue';
import { testingPinia } from '../test-utils/testingPinia';

// The plain sheet, without the RegelRijm reading.
const plugins = () => [testingPinia({ initialState: { featureFlags: { flags: { 'panel.regelrijm': false } } } })];

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
    const wrapper = mount(ActionSheet, { props: { action: ACTION, article: ARTICLE }, attachTo: document.body, global: { plugins: plugins() } });
    await flushPromises();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(wrapper.emitted('close')).toBeUndefined();
  });

  it('closes when the sheet does', async () => {
    const wrapper = mount(ActionSheet, { props: { action: ACTION, article: ARTICLE }, global: { plugins: plugins() } });
    await flushPromises();
    wrapper.find('nldd-sheet').element.dispatchEvent(new CustomEvent('close'));
    expect(wrapper.emitted('close')).toHaveLength(1);
  });
});
