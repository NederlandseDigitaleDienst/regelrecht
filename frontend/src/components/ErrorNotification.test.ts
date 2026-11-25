import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import type { NotificationOptions } from '../lib/raiseNotification';
import ErrorNotification from './ErrorNotification.vue';
import { testingPinia } from '../test-utils/testingPinia';
import { useErrorsStore } from '../stores/errors';

// The design system's `show()`, as far as this component uses it: an element
// that is on screen until dismissed, and the options it was last given.
interface Raised {
  element: HTMLElement;
  options: NotificationOptions;
  onScreen: boolean;
}
const raised = vi.hoisted((): { list: Raised[] } => ({ list: [] }));
vi.mock('../lib/raiseNotification', () => ({
  raiseNotification(options: NotificationOptions) {
    const entry: Raised = { element: document.createElement('div'), options, onScreen: true };
    raised.list.push(entry);
    return {
      element: entry.element,
      dismiss: () => { entry.onScreen = false; },
      update: (next: Partial<NotificationOptions>) => { entry.options = { ...entry.options, ...next }; },
    };
  },
}));

const onScreen = () => raised.list.filter((r) => r.onScreen);
/** Click the notification's action, which also takes it away, as the real one does. */
function clickAction(entry: Raised) {
  entry.onScreen = false;
  entry.options.actions?.[0]?.onClick?.();
}

function mountWith(errors: Array<{ title: string; summary: string; details: string | null }>) {
  const pinia = testingPinia({ stubActions: false });
  const store = useErrorsStore(pinia);
  errors.forEach((error, i) => store.report(`source-${i}`, error.title, error));
  const wrapper = mount(ErrorNotification, { global: { plugins: [pinia] } });
  return { wrapper, store };
}

const SAVE = { title: 'Voorbeeldwet is niet opgeslagen', summary: 'De server heeft het regelwerk geweigerd.', details: 'HTTP 400' };
const ENGINE = { title: 'Voorbeeldwet kan niet worden uitgevoerd', summary: 'Kijk bij artikel 2.', details: 'article 2: ...' };

describe('ErrorNotification', () => {
  beforeEach(() => {
    raised.list = [];
  });

  it('raises nothing without errors', () => {
    mountWith([]);
    expect(raised.list).toEqual([]);
  });

  it('shows one error as itself, with its details behind the button', () => {
    const { store } = mountWith([SAVE]);
    const [notification] = onScreen();
    expect(notification?.options).toMatchObject({
      variant: 'critical',
      text: SAVE.title,
      supportingText: SAVE.summary,
    });
    expect(notification?.options.actions?.map((a) => a.text)).toEqual(['Technische details']);
    clickAction(notification!);
    expect(store.detailsOf).toEqual([SAVE]);
  });

  it('leaves the button out when the sentence is all there is', () => {
    mountWith([{ ...SAVE, details: null }]);
    expect(onScreen()[0]?.options.actions).toEqual([]);
  });

  it('counts several errors in one notification instead of listing them', () => {
    const { store } = mountWith([SAVE, ENGINE]);
    expect(onScreen()).toHaveLength(1);
    const [notification] = onScreen();
    expect(notification?.options.text).toBe('2 foutmeldingen');
    expect(notification?.options.supportingText).not.toContain(SAVE.summary);
    clickAction(notification!);
    expect(store.detailsOf).toEqual([SAVE, ENGINE]);
  });

  it('updates the notification it has when another error joins', async () => {
    const { store } = mountWith([SAVE]);
    store.report('engine', ENGINE.title, ENGINE);
    await nextTick();
    expect(raised.list).toHaveLength(1);
    expect(onScreen()[0]?.options.text).toBe('2 foutmeldingen');
  });

  it('steps aside while the details sheet is open, and comes back after', async () => {
    const { store } = mountWith([SAVE]);
    clickAction(onScreen()[0]!);
    await nextTick();
    expect(onScreen()).toEqual([]);
    store.closeDetails();
    await nextTick();
    expect(onScreen()).toHaveLength(1);
  });

  it('dismisses everything it stands for, and shows a new error after that', async () => {
    const { store } = mountWith([SAVE, ENGINE]);
    const [first] = onScreen();
    // The real element removes itself on its own dismiss.
    first!.onScreen = false;
    first!.element.dispatchEvent(new CustomEvent('dismiss'));
    await nextTick();
    expect(store.visible).toEqual([]);
    expect(onScreen()).toEqual([]);

    store.report('source-0', SAVE.title, { ...SAVE });
    await nextTick();
    expect(onScreen()).toHaveLength(1);
    expect(onScreen()[0]?.options.text).toBe(SAVE.title);
  });

  it('takes its notification along when the shell goes away', () => {
    const { wrapper } = mountWith([SAVE]);
    wrapper.unmount();
    expect(onScreen()).toEqual([]);
  });
});
