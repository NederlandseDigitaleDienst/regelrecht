// RegelRijm in the Machine pane on two small fictional laws: every rule of
// the article as text in full, opening the action at a part, a tooltip on each
// name saying where it comes from, and following a name to the article or
// regelwerk that computes it.
import { describe, it, expect, vi } from 'vitest';
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils';

const WET_B = {
  $id: 'wet_b',
  articles: [{ number: '8', machine_readable: { execution: { actions: [{ output: 'toetsingsinkomen', value: { operation: 'ADD', values: ['$loon', '$winst'] } }] } } }],
};
const ARTIKEL = {
  number: '2',
  machine_readable: {
    definitions: { drempel: { value: 100000 } },
    execution: {
      input: [{ name: 'inkomen', type: 'amount', type_spec: { unit: 'eurocent' }, source: { regulation: 'wet_b', output: 'toetsingsinkomen' } }],
      output: [{ name: 'hoogte', type: 'amount', type_spec: { unit: 'eurocent' } }],
      actions: [
        {
          output: 'hoogte',
          value: { operation: 'MAX', values: [0, { operation: 'MULTIPLY', values: [{ operation: 'SUBTRACT', values: ['$inkomen', '$drempel', '$korting'] }, '$percentage'] }] },
        },
        { output: 'dubbel', value: { operation: 'MULTIPLY', values: ['$hoogte', 2] } },
      ],
    },
  },
};
const WET_A = { $id: 'wet_a', articles: [ARTIKEL, { number: '3', machine_readable: { execution: { actions: [{ output: 'korting', value: 5 }] } } }] };

vi.mock('../composables/useLaw', () => ({
  fetchLaw: vi.fn(async (_traject: string | null, id: string) => {
    if (id === 'wet_c') return { law: { $id: 'wet_c', articles: [] }, lawName: 'Wet C' };
    if (id !== 'wet_b') throw new Error('404');
    return { law: WET_B, lawName: 'Wet B' };
  }),
}));

const { default: RegelRijmPane } = await import('./RegelRijmPane.vue');

async function pane(props = {}) {
  const wrapper = mount(RegelRijmPane, { props: { article: ARTIKEL, law: WET_A, lawId: 'wet_a', ...props } });
  await flushPromises();
  return wrapper;
}

/** The `n`th emit of `event` (the first by default); throws when it is missing. */
function firstEmit(wrapper: VueWrapper, event: string, n = 0): unknown[] {
  const args = wrapper.emitted(event)?.[n];
  if (!args) throw new Error(`expected ${event} to have been emitted`);
  return args;
}

// The reading as a reader sees it: a token shows its text attribute.
function ruleText(wrapper: VueWrapper, output: string) {
  const el = wrapper.find(`[data-testid="regelrijm-pane-rule-${output}"] [data-testid="regelrijm-text"]`).element.cloneNode(true);
  for (const token of (el as Element).querySelectorAll('nldd-token')) token.replaceWith(token.getAttribute('text') ?? '');
  // One element per line of the reading; its depth is in a style, not in the text.
  return [...(el as Element).querySelectorAll('.rr-line')].map((l) => l.textContent).join('\n');
}
const hint = (wrapper: VueWrapper, output: string, ref: string) => wrapper.find(`[data-testid="regelrijm-pane-rule-${output}"] [data-testid="regelrijm-hint-${ref}"]`).attributes('text');

describe('RegelRijmPane', () => {
  it('reads every rule of the article in full', async () => {
    const wrapper = await pane();
    expect(ruleText(wrapper, 'hoogte')).toBe([
      '[hoogte] wordt:',
      'de grootste van (',
      '€ 0,00;',
      '( inkomen min [drempel] min korting ) maal [percentage]',
      ')',
    ].join('\n'));
    expect(ruleText(wrapper, 'dubbel')).toBe('[dubbel] wordt: hoogte maal 2');
    expect(wrapper.text()).not.toContain('⟨');
  });

  it('shows only the heading and the add button when the article has no rules', async () => {
    const wrapper = await pane({ article: { number: '9' }, editable: true });
    expect(wrapper.findAll('[data-testid^="regelrijm-pane-rule-"]')).toHaveLength(0);
    await wrapper.find('[data-testid="add-action-btn"]').trigger('click');
    expect(wrapper.emitted('add-action')).toHaveLength(1);
  });

  it('opens the action at the top from its row', async () => {
    const wrapper = await pane();
    await wrapper.find('[data-testid="regelrijm-pane-open-dubbel"]').trigger('click');
    expect(firstEmit(wrapper, 'open-action')).toEqual([ARTIKEL.machine_readable.execution.actions[1]]);
  });

  it('opens the action at the part whose operation word is clicked', async () => {
    const wrapper = await pane();
    await wrapper.find('[data-testid="regelrijm-pane-rule-hoogte"] [data-testid="regelrijm-select-value/values/1"]').trigger('click');
    const [action, path] = firstEmit(wrapper, 'open-action');
    expect(action).toEqual(ARTIKEL.machine_readable.execution.actions[0]);
    expect(path).toEqual(['value', 'values', 1]);
  });

  it('deletes through the row menu when editable, without opening the row', async () => {
    const wrapper = await pane({ editable: true });
    await wrapper.find('[data-testid="action-dubbel-delete-btn"]').trigger('click');
    expect(firstEmit(wrapper, 'delete-action')).toEqual([1]);
    expect(wrapper.emitted('open-action')).toBeUndefined();
  });

  it('says in a tooltip where each name comes from', async () => {
    const wrapper = await pane();
    expect(hint(wrapper, 'hoogte', '$korting')).toBe('korting: berekend in wet a, artikel 3');
    expect(hint(wrapper, 'hoogte', '$inkomen')).toBe('inkomen: berekend in Wet B, artikel 8');
    expect(hint(wrapper, 'hoogte', '$drempel')).toBe('Vaste waarde in dit artikel: 100000');
    expect(hint(wrapper, 'dubbel', '$hoogte')).toBe('hoogte: berekend in dit artikel');
  });

  it('shows under a rule the fragments of the text linked to it, and nothing without links', async () => {
    const links = new Map([['hoogte', [{ start: 4, quote: 'het verschil', before: 'bedraagt ', after: '.', ellipsisBefore: '…', ellipsisAfter: '' }]]]);
    const wrapper = await pane({ links });
    const text = wrapper.find('[data-testid="regelrijm-pane-text-hoogte"]');
    expect(text.find('nldd-text').text()).toBe('Uit de wettekst');
    expect(text.find('i').text()).toBe('het verschil');
    expect(wrapper.find('[data-testid="regelrijm-pane-text-dubbel"]').exists()).toBe(false);
  });

  it('reads a half-edited article rule by rule instead of failing as a whole', async () => {
    // Half-edited on purpose: shapes the fixture's own type does not allow.
    const article = structuredClone(ARTIKEL) as { machine_readable: { execution: Record<string, unknown> & { actions: unknown[] } } };
    article.machine_readable.execution.actions = [null, { output: 'k', value: { operation: 'IF', cases: 'foo', default: 1 } }, ...article.machine_readable.execution.actions];
    article.machine_readable.execution.parameters = {};
    const wrapper = await pane({ article });
    expect(wrapper.find('[data-testid="regelrijm-pane-rule-0"] h6').text()).toBe('Actie zonder uitkomst');
    expect(ruleText(wrapper, 'k')).toBe('[k] wordt: als ⟨ontbreekt: cases⟩');
    expect(ruleText(wrapper, 'dubbel')).toBe('[dubbel] wordt: hoogte maal 2');
  });

  it('stays in the pane for a rule of the same article, also when the route names the law differently', async () => {
    const wrapper = await pane({ lawId: 'wet-a-in-de-route' });
    await wrapper.find('[data-testid="regelrijm-follow-$hoogte"]').trigger('click');
    await flushPromises();
    expect(wrapper.emitted('navigate')).toBeUndefined();
    await wrapper.find('[data-testid="regelrijm-follow-$korting"]').trigger('click');
    await flushPromises();
    expect(firstEmit(wrapper, 'navigate')[0]).toEqual({ lawId: 'wet-a-in-de-route', articleNumber: '3' });
  });

  it('says why a name in another regelwerk cannot be followed', async () => {
    const article = structuredClone(ARTIKEL) as { machine_readable: { execution: { input: unknown[]; actions: unknown[] } } };
    article.machine_readable.execution.input.push(
      { name: 'weg', source: { regulation: 'wet_c', output: 'weg' } },
      { name: 'kwijt', source: { regulation: 'wet_x', output: 'kwijt' } },
    );
    article.machine_readable.execution.actions.push({ output: 'som', value: { operation: 'ADD', values: ['$weg', '$kwijt'] } });
    const wrapper = await pane({ article });
    await wrapper.find('[data-testid="regelrijm-follow-$weg"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('nldd-banner[variant="warning"]').attributes('text')).toBe('weg is in wet c geen uitkomst van een rekenregel.');
    await wrapper.find('[data-testid="regelrijm-follow-$kwijt"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('nldd-banner[variant="warning"]').attributes('text')).toBe('wet x is niet geladen: 404');
    expect(wrapper.emitted('navigate')).toBeUndefined();
  });

  it('follows a name to the article or regelwerk that computes it', async () => {
    const wrapper = await pane();
    await wrapper.find('[data-testid="regelrijm-follow-$korting"]').trigger('click');
    await flushPromises();
    expect(firstEmit(wrapper, 'navigate')[0]).toEqual({ lawId: 'wet_a', articleNumber: '3' });
    await wrapper.find('[data-testid="regelrijm-follow-$inkomen"]').trigger('click');
    await flushPromises();
    expect(firstEmit(wrapper, 'navigate', 1)[0]).toEqual({ lawId: 'wet_b', articleNumber: '8' });
  });
});
