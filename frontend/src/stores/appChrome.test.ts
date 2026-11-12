import { describe, expect, it, vi } from 'vitest';
import { defineComponent, h, ref } from 'vue';
import { mount } from '@vue/test-utils';
import { useAppChromeStore } from './appChrome';

// The global test setup gives every test a fresh Pinia.
describe('useAppChromeStore', () => {
  it('drops everything the editor published when it clears its chrome', () => {
    const chrome = useAppChromeStore();
    chrome.setEditorChrome({ pr: { number: 7, url: 'https://example.test/pr/7' }, tabs: [{ lawId: 'wet_a', articleNumber: '1' }] });
    chrome.setEditorChanges({ dirty: true, saving: false, canUndo: false, canRedo: false, review: false, reviewStatus: null, reviewCanRejectRest: false });

    chrome.clearEditorChrome();

    expect(chrome.lastSavedPr).toBeNull();
    expect(chrome.documentTabs).toEqual([]);
    expect(chrome.editorChanges).toBeNull();
  });

  it('opens the search popover of the view that registered it, until that view unmounts', () => {
    const show = vi.fn();
    const View = defineComponent({
      setup() {
        useAppChromeStore().registerSearchPopover(ref({ show }));
        return () => h('div');
      },
    });
    const chrome = useAppChromeStore();
    const view = mount(View);

    chrome.openSearch(undefined, 'zorg');
    expect(show).toHaveBeenCalledWith(undefined, 'zorg', undefined);

    view.unmount();
    chrome.openSearch(undefined, 'weer');
    expect(show).toHaveBeenCalledTimes(1);
  });
});
