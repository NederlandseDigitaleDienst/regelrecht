import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import ExecutionTraceView from './ExecutionTraceView.vue';
import { testingPinia } from '../test-utils/testingPinia';
import { useErrorsStore } from '../stores/errors';
import { GO_TO_FINDING } from '../lib/engineDiagnostics';

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

  it('keeps the sentence above the trace when the engine gave a partial trace', () => {
    const pinia = testingPinia();
    const wrapper = mount(ExecutionTraceView, {
      props: { error: ENGINE_TEXT, traceText: 'Execution failed: the same text again' },
      global: { plugins: [pinia] },
    });
    expect(wrapper.find('[data-testid="execution-error"]').attributes('supporting-text')).toBe('Het scenario kon niet worden uitgevoerd.');
    expect(wrapper.find('nldd-code-viewer').text()).toContain('Execution failed');
  });

  it('names the article of a marking nobody accepted, and hands the way to it to the sheet', async () => {
    const marking = {
          severity: 'error' as const,
          kind: 'marking' as const,
          law_id: 'wet_voorbeeld',
          article: '5',
          place: { kind: 'article' as const },
          position: null,
          rule: null,
          flagged: {
            about: 'een code beginnend met 011',
            reason: 'De engine vergelijkt teksten alleen in hun geheel.',
            resolution: 'operation' as const,
            resolved_by: 'Een bewerking STARTS_WITH',
          },
          message: 'Untranslatable construct in wet_voorbeeld article 5: een code beginnend met 011 — …',
        };
    const pinia = testingPinia();
    const goTo = () => {};
    const wrapper = mount(ExecutionTraceView, {
      props: { error: marking.message, diagnostics: [marking] },
      global: { plugins: [pinia], provide: { [GO_TO_FINDING as symbol]: goTo } },
    });
    const title = 'Artikel 5 van Wet Voorbeeld heeft een markering die nog niet is geaccepteerd';
    expect(wrapper.find('[data-testid="execution-error"]').attributes('text')).toBe(title);
    await wrapper.find('[data-testid="execution-error-details"]').trigger('click');
    expect(useErrorsStore(pinia).showDetails).toHaveBeenCalledWith([
      expect.objectContaining({ title, diagnostics: [marking], goTo }),
    ]);
  });
});
