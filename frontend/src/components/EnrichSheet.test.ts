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

function mountArticle(stepped = true) {
  return mount(EnrichSheet, {
    props: {
      mode: 'article',
      law: { articles: [{ number: '2' }] },
      lawId: 'zorgtoeslag',
      trajectRef: 'traject-abcd1234',
      article: '2',
      stepped,
    },
    attachTo: document.body,
  });
}

function click(testid: string) {
  (document.querySelector(`[data-testid="${testid}"]`) as HTMLElement).click();
}

describe('EnrichSheet in the article mode', () => {
  beforeEach(() => {
    apiFetchJson.mockReset();
    requestEnrichPlan.mockReset();
    apiFetchJson.mockImplementation(async (url: string) =>
      url.includes('/chain') ? CHAIN : { consumers: [], partial: false },
    );
    document.body.innerHTML = '';
  });

  it('suggests what on the chain could use enrichment, ticked off, and asks the model nothing', async () => {
    mountArticle();
    await flushPromises();
    expect(apiFetchJson).toHaveBeenCalledWith(
      '/api/trajects/traject-abcd1234/corpus/laws/zorgtoeslag/articles/2/chain?depth=3',
    );
    const rows = [...document.querySelectorAll('[data-testid="chain-suggestion"] nldd-checkbox-field')];
    // Awir 9 has a model and no hotspot: no reason to suggest it.
    expect(rows.map((r) => r.getAttribute('label'))).toEqual(['Awir art. 8']);
    expect(rows[0]?.hasAttribute('checked')).toBe(false);
    expect(requestEnrichPlan).not.toHaveBeenCalled();
  });

  it('starts the article alone by default, with the stelselcontext', async () => {
    const wrapper = mountArticle();
    await flushPromises();
    click('enrich-start');
    await flushPromises();
    expect(requestEnrichPlan).not.toHaveBeenCalled();
    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({ mode: 'article', articles: ['2'], context: true });
  });

  it('sends one request per law once a suggestion is ticked', async () => {
    requestEnrichPlan.mockResolvedValue({ started: ['zorgtoeslag', 'awir'], conflicts: [], tooMany: [], failed: [] });
    const wrapper = mountArticle();
    await flushPromises();
    const box = document.querySelector('[data-testid="chain-suggestion"] nldd-checkbox-field') as HTMLElement;
    box.dispatchEvent(new CustomEvent('change', { detail: { checked: true } }));
    await flushPromises();
    click('enrich-start');
    await flushPromises();
    expect(requestEnrichPlan).toHaveBeenCalledWith('traject-abcd1234', [
      { lawId: 'zorgtoeslag', options: { mode: 'article', articles: ['2'], instruction: undefined, context: true } },
      { lawId: 'awir', options: { mode: 'law', articles: ['8'], context: true } },
    ]);
    expect(wrapper.emitted('submitted')).toHaveLength(1);
    expect(wrapper.emitted('close')).toHaveLength(1);
  });

  it('without the chain flag, fetches no chain', async () => {
    mountArticle(false);
    await flushPromises();
    expect(apiFetchJson).not.toHaveBeenCalled();
  });
});

describe('EnrichSheet in the law mode', () => {
  beforeEach(() => {
    apiFetchJson.mockReset();
    document.body.innerHTML = '';
  });

  it('says in one line what is enriched, and sends only what has no model', async () => {
    const wrapper = mount(EnrichSheet, {
      props: {
        mode: 'law',
        law: { articles: [{ number: '1', machine_readable: {} }, { number: '2' }, { number: '3' }] },
        lawId: 'zorgtoeslag',
        trajectRef: 'traject-abcd1234',
      },
      attachTo: document.body,
    });
    await flushPromises();
    expect(document.querySelector('[data-testid="law-summary"]')?.getAttribute('text')).toBe(
      '2 van de 3 artikelen hebben nog geen machine_readable. Die worden verrijkt.',
    );
    // The article list stays closed until asked for.
    expect(document.body.textContent).not.toContain('Alle artikelen');
    click('enrich-start');
    await flushPromises();
    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({
      mode: 'law',
      articles: ['1', '2', '3'],
      onlyMissing: true,
      context: true,
    });
  });
});
