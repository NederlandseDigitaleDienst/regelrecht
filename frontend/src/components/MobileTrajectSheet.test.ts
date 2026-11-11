import { describe, it, expect, beforeAll, beforeEach, afterEach, vi } from 'vitest';
import type { Mock } from 'vitest';
import { mount } from '@vue/test-utils';

// The component (and the trajecten store) use vue-router; provide a light mock so it
// mounts without a real router. The full traject-list / create / auth flows
// are ported verbatim from TrajectMenu + DocumentTabsSheet and verified there.
const pushMock = vi.fn();
vi.mock('vue-router', () => ({
  useRoute: () => ({ params: {}, name: 'editor-traject' }),
  useRouter: () => ({ push: pushMock }),
}));

import MobileTrajectSheet from './MobileTrajectSheet.vue';
import {
  setEditorChrome,
  registerTabActions,
  clearEditorChrome,
} from '../composables/useAppChrome';
import type { OpenTab } from '../lib/openTabsStorage';

// nldd-sheet compiles to a raw custom element; stub show()/hide() with spies.
let showSpy: Mock<() => void> | undefined, hideSpy: Mock<() => void> | undefined;
beforeAll(() => {
  if (typeof customElements !== 'undefined' && !customElements.get('nldd-sheet')) {
    class NddSheetTestStub extends HTMLElement {
      show() { showSpy?.(); }
      hide() { hideSpy?.(); }
    }
    customElements.define('nldd-sheet', NddSheetTestStub);
  }
});

const TABS: OpenTab[] = [
  { lawId: 'zorgtoeslagwet', articleNumber: '3' },
  { lawId: 'awir', articleNumber: '1' },
];
const NAMES: Record<string, string> = { zorgtoeslagwet: 'Zorgtoeslagwet', awir: 'Awir' };
const tabKey = (t: OpenTab) => `${t.lawId}:${t.articleNumber}`;

beforeEach(() => {
  showSpy = vi.fn();
  hideSpy = vi.fn();
  registerTabActions({
    key: tabKey,
    displayName: (t: OpenTab) => NAMES[t.lawId] || t.lawId,
    select: vi.fn(),
    close: vi.fn(),
    reorder: vi.fn(),
  });
  setEditorChrome({ pr: null, tabs: TABS, activeTab: TABS[0] });
});

afterEach(() => clearEditorChrome());

function mountSheet() {
  return mount(MobileTrajectSheet, {
    attachTo: document.body,
    global: { stubs: { teleport: true } },
  });
}

describe('MobileTrajectSheet', () => {
  it('mounts and shows the active article · law on the trigger button', () => {
    const wrapper = mountSheet();
    // The trigger is the first nldd-button in the template.
    const btn = wrapper.get('nldd-button');
    expect(btn.attributes('text')).toBe('Artikel 3 · Zorgtoeslagwet');
  });

  it('opens the sheet on trigger click', async () => {
    const wrapper = mountSheet();
    await wrapper.get('nldd-button').trigger('click');
    expect(showSpy).toHaveBeenCalled();
  });

  it('falls back to "Trajecten" on the button when no tab and no traject', () => {
    clearEditorChrome();
    setEditorChrome({ pr: null, tabs: [], activeTab: null });
    const wrapper = mountSheet();
    expect(wrapper.get('nldd-button').attributes('text')).toBe('Trajecten');
  });

  it('closes the sheet when the viewport grows past sm', () => {
    // Typed via `as` so TS does not narrow it to `null` (it is set in a callback).
    let changeHandler = null as ((e: { matches: boolean }) => void) | null;
    // A partial MediaQueryList: only what the component touches.
    const spy = vi.spyOn(window, 'matchMedia').mockReturnValue({
      matches: false,
      addEventListener: (event: string, handler: (e: { matches: boolean }) => void) => {
        if (event === 'change') changeHandler = handler;
      },
      removeEventListener: () => {},
    } as unknown as MediaQueryList);
    try {
      mountSheet();
      changeHandler?.({ matches: true });
      expect(hideSpy).toHaveBeenCalled();
    } finally {
      spy.mockRestore();
    }
  });
});
