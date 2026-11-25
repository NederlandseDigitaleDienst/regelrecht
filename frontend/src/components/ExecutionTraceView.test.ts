import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import ExecutionTraceView from './ExecutionTraceView.vue';
import { testingPinia } from '../test-utils/testingPinia';
import { useErrorsStore } from '../stores/errors';

const ENGINE_TEXT = 'Resolution error: nothing matched the reference in a very long engine message';

function mountWithError(error: string) {
  const pinia = testingPinia();
  const wrapper = mount(ExecutionTraceView, { props: { error }, global: { plugins: [pinia] } });
  return { wrapper, store: useErrorsStore(pinia) };
}

describe('ExecutionTraceView, a failed run', () => {
  it('shows a readable sentence instead of the engine text', () => {
    const { wrapper } = mountWithError(ENGINE_TEXT);
    const dialog = wrapper.find('nldd-inline-dialog');
    expect(dialog.attributes('supporting-text')).toBe('Het scenario kon niet worden uitgevoerd.');
    expect(wrapper.html()).not.toContain('Resolution error');
  });

  it('hands the engine text to the details sheet', async () => {
    const { wrapper, store } = mountWithError(ENGINE_TEXT);
    await wrapper.find('[data-testid="execution-error-details"]').trigger('click');
    expect(store.showDetails).toHaveBeenCalledWith([
      {
        title: 'Fout bij uitvoering',
        summary: 'Het scenario kon niet worden uitgevoerd.',
        details: ENGINE_TEXT,
      },
    ]);
  });
});
