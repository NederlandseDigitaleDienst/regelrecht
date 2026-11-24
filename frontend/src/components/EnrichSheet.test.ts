import { describe, it, expect, vi, beforeAll, beforeEach } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import EnrichSheet from './EnrichSheet.vue';
import type { Chain } from '../lib/enrichChain';

// nldd-sheet is driven with show()/hide(); the custom elements are not
// registered in tests (same stub as TasksListPane.test.ts).
beforeAll(() => {
  if (typeof customElements !== 'undefined' && !customElements.get('nldd-sheet')) {
    class SheetStub extends HTMLElement {
      show() {}
      hide() {}
    }
    customElements.define('nldd-sheet', SheetStub);
  }
});

const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch', async (importOriginal) => ({
  ...(await importOriginal()),
  apiFetchJson: (...a: unknown[]) => apiFetchJson(...a),
}));

const requestEnrichPlan = vi.fn();
vi.mock('../composables/useTasks', async (importOriginal) => {
  const real = await importOriginal<typeof import('../composables/useTasks')>();
  const { ref } = await import('vue');
  return {
    ...real,
    useTaskActions: () => ({ requestEnrichPlan, running: ref([]) }),
  };
});

const CHAIN: Chain = {
  root: { law_id: 'zorgtoeslag', article: '2' },
  nodes: [
    {
      law_id: 'zorgtoeslag',
      bwb_id: 'BWBZ',
      article: '2',
      depth: 0,
      has_mr: true,
      gaps: { missing_mr: false, dangling: 0, unresolved: 0, open_terms: 0 },
      via: null,
      hotspot: null,
    },
    {
      law_id: 'awir',
      bwb_id: 'BWBA',
      article: '8',
      depth: 1,
      has_mr: false,
      gaps: { missing_mr: true, dangling: 0, unresolved: 0, open_terms: 0 },
      via: { law_id: 'zorgtoeslag', article: '2', kind: 'binds' },
      hotspot: { score: 6, reasons: [{ signal: 'M', points: 3, detail: 'Geen machine_readable', refs: [] }] },
    },
    {
      law_id: 'awir',
      bwb_id: 'BWBA',
      article: '9',
      depth: 1,
      has_mr: true,
      gaps: { missing_mr: false, dangling: 0, unresolved: 0, open_terms: 0 },
      via: { law_id: 'zorgtoeslag', article: '2', kind: 'cites' },
      hotspot: null,
    },
  ],
  gaps: [],
  partial: { unread_laws: 0, depth_capped: false, node_capped: false },
};

function mountStepped() {
  return mount(EnrichSheet, {
    props: {
      mode: 'article',
      law: { articles: [{ number: '2' }] },
      lawId: 'zorgtoeslag',
      trajectRef: 'traject-abcd1234',
      article: '2',
      stepped: true,
    },
    attachTo: document.body,
  });
}

describe('EnrichSheet in steps', () => {
  beforeEach(() => {
    apiFetchJson.mockReset();
    requestEnrichPlan.mockReset();
    apiFetchJson.mockImplementation(async (url: string) =>
      url.includes('/chain') ? CHAIN : { consumers: [], partial: false },
    );
    document.body.innerHTML = '';
  });

  it('shows the chain first and asks the model nothing until the last button', async () => {
    mountStepped();
    await flushPromises();
    expect(apiFetchJson).toHaveBeenCalledWith(
      '/api/trajects/traject-abcd1234/corpus/laws/zorgtoeslag/articles/2/chain?depth=3',
    );
    const bar = document.querySelector('nldd-top-title-bar');
    expect(bar?.getAttribute('supporting-text')).toBe('Stap 1 van 3');
    expect(document.body.textContent).toContain('Artikel 8');
    expect(requestEnrichPlan).not.toHaveBeenCalled();
  });

  it('preselects the hotspots, and sends one request per law at the end', async () => {
    requestEnrichPlan.mockResolvedValue({ started: ['zorgtoeslag', 'awir'], conflicts: [], tooMany: [], failed: [] });
    const wrapper = mountStepped();
    await flushPromises();
    (document.querySelector('[data-testid="step-next"]') as HTMLElement).click();
    await flushPromises();
    const checked = [...document.querySelectorAll('nldd-checkbox-field[checked]')].map((c) => c.getAttribute('label'));
    expect(checked).toEqual(['Artikel 2', 'Artikel 8']);

    (document.querySelector('[data-testid="step-next"]') as HTMLElement).click();
    await flushPromises();
    (document.querySelector('[data-testid="step-start"]') as HTMLElement).click();
    await flushPromises();
    expect(requestEnrichPlan).toHaveBeenCalledWith('traject-abcd1234', [
      { lawId: 'zorgtoeslag', options: { mode: 'article', articles: ['2'], instruction: undefined, context: true } },
      { lawId: 'awir', options: { mode: 'law', articles: ['8'], context: true } },
    ]);
    expect(wrapper.emitted('submitted')).toHaveLength(1);
    expect(wrapper.emitted('close')).toHaveLength(1);
  });
});
