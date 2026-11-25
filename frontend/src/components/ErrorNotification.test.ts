import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import ErrorNotification from './ErrorNotification.vue';
import { testingPinia } from '../test-utils/testingPinia';
import { useErrorsStore } from '../stores/errors';

function mountWith(errors: Array<{ title: string; summary: string; details: string | null }>) {
  const pinia = testingPinia({ stubActions: false });
  const store = useErrorsStore(pinia);
  errors.forEach((error, i) => store.report(`source-${i}`, error.title, error));
  const wrapper = mount(ErrorNotification, { global: { plugins: [pinia] } });
  return { wrapper, store, notification: () => wrapper.find('[data-testid="error-notification"]') };
}

const SAVE = { title: 'Opslaan mislukt', summary: 'De server heeft het regelwerk geweigerd.', details: 'HTTP 400' };
const ENGINE = { title: 'Dit regelwerk kan niet worden uitgevoerd', summary: 'Kijk bij artikel 2.', details: 'article 2: ...' };

describe('ErrorNotification', () => {
  it('renders nothing without errors', () => {
    expect(mountWith([]).notification().exists()).toBe(false);
  });

  it('shows one error as itself, with its details behind the button', async () => {
    const { wrapper, store, notification } = mountWith([SAVE]);
    expect(notification().attributes('text')).toBe('Opslaan mislukt');
    expect(notification().attributes('supporting-text')).toBe(SAVE.summary);
    await wrapper.find('[data-testid="error-notification-details"]').trigger('click');
    expect(store.detailsOf).toEqual([SAVE]);
  });

  it('leaves the button out when the sentence is all there is', () => {
    const { wrapper } = mountWith([{ ...SAVE, details: null }]);
    expect(wrapper.find('[data-testid="error-notification-details"]').exists()).toBe(false);
  });

  it('counts several errors instead of listing them', async () => {
    const { wrapper, store, notification } = mountWith([SAVE, ENGINE]);
    expect(wrapper.findAll('nldd-notification')).toHaveLength(1);
    expect(notification().attributes('text')).toBe('2 foutmeldingen');
    expect(notification().attributes('supporting-text')).not.toContain(SAVE.summary);
    await wrapper.find('[data-testid="error-notification-details"]').trigger('click');
    expect(store.detailsOf).toEqual([SAVE, ENGINE]);
  });

  it('steps aside while the details sheet is open', async () => {
    const { wrapper, store, notification } = mountWith([SAVE]);
    await wrapper.find('[data-testid="error-notification-details"]').trigger('click');
    expect(notification().exists()).toBe(false);
    store.closeDetails();
    await wrapper.vm.$nextTick();
    expect(notification().exists()).toBe(true);
  });

  it('dismisses everything it stands for', async () => {
    const { notification } = mountWith([SAVE, ENGINE]);
    await notification().trigger('dismiss');
    expect(notification().exists()).toBe(false);
  });
});
