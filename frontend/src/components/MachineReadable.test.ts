import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import type { VueWrapper } from '@vue/test-utils';
import MachineReadable from './MachineReadable.vue';

type ArticleProp = NonNullable<InstanceType<typeof MachineReadable>['$props']['article']>;
type MachineReadableSection = NonNullable<ArticleProp['machine_readable']>;
/** The fixture: an article whose machine_readable and execution are always present. */
type ArticleFixture = ArticleProp & {
  machine_readable: MachineReadableSection & {
    execution: NonNullable<MachineReadableSection['execution']>;
  };
};

/** An emitted payload, typed for the fields these tests read. */
interface EmitPayload {
  section?: string;
  key?: string;
  index?: number;
  isNew?: boolean;
  rawDef?: unknown;
  output?: string;
  data?: { name?: string; source?: { regulation?: string }; type_spec?: unknown };
}

/** The first argument of the first emit; throws when nothing was emitted. */
function firstPayload(events: unknown[][] | undefined): EmitPayload {
  const args = events?.[0];
  if (!args) throw new Error('expected the event to have been emitted');
  return args[0] as EmitPayload;
}

// Minimal article fixture with all section types
function createArticle(overrides: Omit<Partial<MachineReadableSection>, 'execution'> = {}): ArticleFixture {
  // Built as a plain object first: it carries law-format fields the component
  // does not read (source.output, operation, ...), which a direct return
  // would reject as excess properties.
  const article = {
    number: '2',
    machine_readable: {
      definitions: {
        drempelinkomen: { value: 3971900 },
        percentage_drempel: { value: 0.01896 },
        standaard_bedrag: { value: 150000, type_spec: { unit: 'eurocent' } },
      },
      execution: {
        produces: {
          legal_character: 'BESCHIKKING',
          decision_type: 'TOEKENNING',
        },
        parameters: [
          { name: 'bsn', type: 'string', required: true },
        ],
        input: [
          {
            name: 'leeftijd',
            type: 'number',
            source: {
              regulation: 'wet_brp',
              output: 'leeftijd',
              parameters: { bsn: '$bsn' },
            },
          },
          {
            name: 'is_verzekerde',
            type: 'boolean',
            source: {
              regulation: 'zorgverzekeringswet',
              output: 'is_verzekerd',
            },
          },
        ],
        output: [
          { name: 'heeft_recht', type: 'boolean' },
          { name: 'hoogte', type: 'amount', type_spec: { unit: 'eurocent' } },
        ],
        actions: [
          { output: 'hoogte', value: 0, operation: { type: 'LITERAL', value: 0 } },
        ],
      },
      ...overrides,
    },
  };
  return article;
}

function mountEditable(articleOverrides: Omit<Partial<MachineReadableSection>, 'execution'> = {}) {
  return mount(MachineReadable, {
    props: {
      article: createArticle(articleOverrides),
      editable: true,
    },
  });
}

// "Bewerk" moved from an inline nldd-button to a RowActionsMenu menu-item
// (the more-menu). In mount() custom elements aren't upgraded, so the
// menu-item is just a DOM node and triggering click still fires the
// component's @click → emit('edit'). DOM order per row is unchanged, so
// the index mapping used by the tests is preserved.
function findBewerkButtons(wrapper: VueWrapper) {
  return wrapper.findAll('nldd-menu-item[text="Bewerk"]');
}

function findBekijkButtons(wrapper: VueWrapper) {
  return wrapper.findAll('nldd-button[text="Bekijk"]');
}

async function clickBewerk(wrapper: VueWrapper, index: number) {
  const buttons = findBewerkButtons(wrapper);
  await buttons.at(index)?.trigger('click');
}

// Add buttons all carry a stable data-testid (`add-{section}-btn`); using
// it sidesteps copy churn - we changed labels from "Nieuwe X" to
// "X toevoegen" and don't want the test to break on the next reword.
const ADD_BTN_TESTID: Record<string, string> = {
  definitie: 'add-def-btn',
  parameter: 'add-param-btn',
  input: 'add-input-btn',
  output: 'add-output-btn',
  action: 'add-action-btn',
};
function findAddButton(wrapper: VueWrapper, section: string) {
  return wrapper.find(`[data-testid="${ADD_BTN_TESTID[section]}"]`);
}

// The destructive button in the confirm modal that actually emits the
// delete event. There's one per pendingDelete (the modal renders only
// when something is staged) so a single global lookup is fine.
function findConfirmDeleteButton(wrapper: VueWrapper) {
  return wrapper.find('nldd-button[appearance="destructive"][text="Verwijder"]');
}

describe('MachineReadable', () => {
  describe('display mode', () => {
    it('renders all sections', () => {
      const wrapper = mountEditable();
      const headings = wrapper.findAll('nldd-title');
      const titles = headings.map((h) => h.text());
      expect(titles).toContain('Definities');
      expect(titles).toContain('Parameters');
      expect(titles).toContain('Inputs');
      expect(titles).toContain('Outputs');
      expect(titles).toContain('Acties');
    });

    it('shows produces metadata as editable dropdowns', () => {
      const wrapper = mountEditable();
      expect(wrapper.find('select[aria-label="Juridische basis"]').exists()).toBe(true);
      expect(wrapper.find('select[aria-label="Besluit-type"]').exists()).toBe(true);
      const opts = wrapper.findAll('option').map((o) => o.text());
      expect(opts).toContain('beschikking');
      expect(opts).toContain('toekenning');
    });

    it('shows the decision of a produces list (schema v0.8.1)', () => {
      const article = createArticle();
      article.machine_readable.execution.produces = [
        { type: 'AANVRAAG', requests: 'test#1', parameters: ['bsn'] },
        { type: 'BESCHIKKING', decision_type: 'AFWIJZING', outputs: ['heeft_recht'] },
      ];
      const wrapper = mount(MachineReadable, { props: { article, editable: true } });
      expect(wrapper.find<HTMLSelectElement>('select[aria-label="Juridische basis"]').element.value).toBe('BESCHIKKING');
      expect(wrapper.find<HTMLSelectElement>('select[aria-label="Besluit-type"]').element.value).toBe('AFWIJZING');
    });

    it('offers no decision type for a produces list without a decision', () => {
      const article = createArticle();
      article.machine_readable.execution.produces = [
        { type: 'AANVRAAG', requests: 'test#1', parameters: ['bsn'] },
      ];
      const wrapper = mount(MachineReadable, { props: { article, editable: true } });
      expect(wrapper.find<HTMLSelectElement>('select[aria-label="Juridische basis"]').element.value).toBe('');
      expect(wrapper.find('select[aria-label="Besluit-type"]').exists()).toBe(false);
    });

    it('shows a produces object as written, also without a legal character', () => {
      const article = createArticle();
      article.machine_readable.execution.produces = { decision_type: 'TOEKENNING' };
      const read = mount(MachineReadable, { props: { article, editable: false } });
      expect(read.findAll('nldd-text-cell').some((c) => c.attributes('text') === 'Besluit-type')).toBe(true);
      const edit = mount(MachineReadable, { props: { article, editable: true } });
      expect(edit.find<HTMLSelectElement>('select[aria-label="Besluit-type"]').element.value).toBe('TOEKENNING');
    });

    it('shows empty state when no machine_readable', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: { number: '1' }, editable: true },
      });
      expect(wrapper.find('nldd-inline-dialog').attributes('text')).toContain('Geen machine-leesbare gegevens');
    });

    // Nothing in `{ value: 0.01896 }` says it is a percentage, so it is not
    // shown as one: the earlier guess for every number between 0 and 1 hid a
    // factor of 100. A ratio says so and is shown with what it amounts to.
    it('shows a number between 0 and 1 without a unit as the number it is', () => {
      const wrapper = mountEditable();
      // The value sits in the `text` attribute (read-only cells) or as
      // slot text next to BreakableName (editable definition rows).
      const cells = wrapper.findAll('nldd-text-cell').map(c => `${c.attributes('text') || ''} ${c.text()}`);
      expect(cells.some(t => /0\.01896/.test(t))).toBe(true);
      expect(cells.some(t => /1,896\s*%/.test(t))).toBe(false);
    });

    it('shows a ratio with the percentage it amounts to', () => {
      const wrapper = mount(MachineReadable, {
        props: {
          article: createArticle({
            definitions: { aandeel: { value: 0.01896, type: 'number', type_spec: { unit: 'ratio' } } },
          }),
          editable: true,
        },
      });
      const cells = wrapper.findAll('nldd-text-cell').map(c => `${c.attributes('text') || ''} ${c.text()}`);
      expect(cells.some(t => /0,01896 \(1,896%\)/.test(t))).toBe(true);
    });

    it('formats eurocent values as currency', () => {
      const wrapper = mountEditable();
      // The value sits in the `text` attribute (read-only cells) or as
      // slot text next to BreakableName (editable definition rows).
      const cells = wrapper.findAll('nldd-text-cell').map(c => `${c.attributes('text') || ''} ${c.text()}`);
      expect(cells.some(t => /1\.500,00/.test(t))).toBe(true);
    });

    it('shows plain number when no unit', () => {
      const wrapper = mountEditable();
      // The value sits in the `text` attribute (read-only cells) or as
      // slot text next to BreakableName (editable definition rows).
      const cells = wrapper.findAll('nldd-text-cell').map(c => `${c.attributes('text') || ''} ${c.text()}`);
      expect(cells.some(t => t.includes('3971900'))).toBe(true);
    });

    it('renders a YAML-null definition as an empty value', () => {
      // `definitions: { leeg: ~ }` parses to null.
      for (const editable of [true, false]) {
        const wrapper = mount(MachineReadable, {
          props: { article: createArticle({ definitions: { leeg: null } }), editable },
        });
        const cells = wrapper.findAll('nldd-text-cell').map(c => `${c.attributes('text') || ''} ${c.text()}`);
        expect(cells.some(t => t.includes('leeg'))).toBe(true);
        expect(cells.some(t => t.includes('null'))).toBe(false);
      }
    });

    it('shows Bewerk buttons for each editable item', () => {
      const wrapper = mountEditable();
      const buttons = findBewerkButtons(wrapper);
      // 3 defs + 1 param + 2 inputs + 2 outputs + 1 action = 9
      expect(buttons.length).toBe(9);
    });
  });

  describe('definition editing', () => {
    it('emits open-edit with definition data on Bewerk click', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 0);
      const events = wrapper.emitted('open-edit');
      expect(events).toHaveLength(1);
      expect(firstPayload(events).section).toBe('definition');
      expect(firstPayload(events).key).toBe('drempelinkomen');
      expect(firstPayload(events).rawDef).toEqual({ value: 3971900 });
    });

    it('emits open-edit for eurocent definition', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 2);
      const events = wrapper.emitted('open-edit');
      expect(firstPayload(events).key).toBe('standaard_bedrag');
      expect(firstPayload(events).rawDef).toEqual({ value: 150000, type_spec: { unit: 'eurocent' } });
    });
  });

  describe('parameter editing', () => {
    it('emits open-edit with parameter data on Bewerk click', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 3);
      const events = wrapper.emitted('open-edit');
      expect(events).toHaveLength(1);
      expect(firstPayload(events)).toEqual({
        section: 'parameter',
        index: 0,
        data: { name: 'bsn', type: 'string', required: true },
      });
    });
  });

  describe('input editing', () => {
    it('emits open-edit with input data on Bewerk click', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 4);
      const events = wrapper.emitted('open-edit');
      expect(events).toHaveLength(1);
      expect(firstPayload(events).section).toBe('input');
      expect(firstPayload(events).index).toBe(0);
      expect(firstPayload(events).data?.name).toBe('leeftijd');
      expect(firstPayload(events).data?.source?.regulation).toBe('wet_brp');
    });
  });

  describe('output editing', () => {
    it('emits open-edit with output data on Bewerk click', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 6);
      const events = wrapper.emitted('open-edit');
      expect(events).toHaveLength(1);
      expect(firstPayload(events)).toEqual({
        section: 'output',
        index: 0,
        data: { name: 'heeft_recht', type: 'boolean' },
      });
    });

    it('emits open-edit preserving type_spec', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 7);
      const events = wrapper.emitted('open-edit');
      expect(firstPayload(events).data?.type_spec).toEqual({ unit: 'eurocent' });
    });
  });

  describe('actions', () => {
    it('emits open-action on Bewerk click', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 8);
      const events = wrapper.emitted('open-action');
      expect(events).toHaveLength(1);
      expect(firstPayload(events).output).toBe('hoogte');
    });
  });

  describe('adding new items', () => {
    it('shows add buttons for all sections when editable', () => {
      const wrapper = mountEditable();
      expect(findAddButton(wrapper, 'definitie').exists()).toBe(true);
      expect(findAddButton(wrapper, 'parameter').exists()).toBe(true);
      expect(findAddButton(wrapper, 'input').exists()).toBe(true);
      expect(findAddButton(wrapper, 'output').exists()).toBe(true);
    });

    it('does not show add buttons when not editable', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      // No add-X-btn data-testid should be present in read-only mode.
      const addButtons = wrapper.findAll('[data-testid^="add-"][data-testid$="-btn"]');
      expect(addButtons.length).toBe(0);
    });

    it('emits open-edit for new definition', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'definitie').trigger('click');
      const events = wrapper.emitted('open-edit');
      expect(firstPayload(events).section).toBe('add-definition');
      expect(firstPayload(events).isNew).toBe(true);
    });

    it('emits open-edit for new parameter', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'parameter').trigger('click');
      const events = wrapper.emitted('open-edit');
      expect(firstPayload(events).section).toBe('add-parameter');
      expect(firstPayload(events).isNew).toBe(true);
    });

    it('emits open-edit for new input', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'input').trigger('click');
      const events = wrapper.emitted('open-edit');
      expect(firstPayload(events).section).toBe('add-input');
      expect(firstPayload(events).isNew).toBe(true);
    });

    it('emits open-edit for new output', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'output').trigger('click');
      const events = wrapper.emitted('open-edit');
      expect(firstPayload(events).section).toBe('add-output');
      expect(firstPayload(events).isNew).toBe(true);
    });
  });

  describe('non-editable mode', () => {
    it('renders action rows as button-type list items with a chevron', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      // No Bewerk/Bekijk buttons in read-only mode; list-item itself is the trigger.
      expect(findBekijkButtons(wrapper).length).toBe(0);
      const actionItems = wrapper.findAll('nldd-list-item[button]');
      expect(actionItems.length).toBeGreaterThan(0);
    });

    it('clicking an action row emits open-action', async () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      const actionItems = wrapper.findAll('nldd-list-item[button]');
      await actionItems.at(0)?.trigger('click');
      expect(wrapper.emitted('open-action')).toHaveLength(1);
      expect(wrapper.emitted('open-edit')).toBeUndefined();
    });
  });

  // Save failures no longer render inline here; EditorView surfaces lawSaveError
  // as a single modal over the whole editor.

  describe('delete row buttons', () => {
    function findDeleteByTestId(wrapper: VueWrapper, testid: string) {
      return wrapper.find(`[data-testid="${testid}"]`);
    }

    it('renders a minus icon button next to every editable row when editable', () => {
      const wrapper = mountEditable();
      // 3 definitions + 1 parameter + 2 inputs + 2 outputs + 1 action = 9
      expect(wrapper.findAll('[data-testid$="-delete-btn"]')).toHaveLength(9);
    });

    it('does not render delete buttons in read-only mode', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      expect(wrapper.findAll('[data-testid$="-delete-btn"]')).toHaveLength(0);
    });

    // The minus icon now stages a confirmation in `pendingDelete` and the
    // destructive button in the modal-dialog is what actually emits the
    // delete event. Each test exercises that two-step flow so we cover the
    // user-facing contract, not the internal staging state.
    async function clickDeleteAndConfirm(wrapper: VueWrapper, testid: string) {
      await findDeleteByTestId(wrapper, testid).trigger('click');
      await findConfirmDeleteButton(wrapper).trigger('click');
    }

    it('emits delete with definition payload after confirming', async () => {
      const wrapper = mountEditable();
      await clickDeleteAndConfirm(wrapper, 'def-drempelinkomen-delete-btn');
      const events = wrapper.emitted('delete');
      expect(events).toHaveLength(1);
      expect(firstPayload(events)).toEqual({ section: 'definition', key: 'drempelinkomen' });
    });

    it('does not emit delete on the minus click alone (waits for confirmation)', async () => {
      const wrapper = mountEditable();
      await findDeleteByTestId(wrapper, 'def-drempelinkomen-delete-btn').trigger('click');
      expect(wrapper.emitted('delete')).toBeUndefined();
    });

    it('emits delete with parameter index payload after confirming', async () => {
      const wrapper = mountEditable();
      await clickDeleteAndConfirm(wrapper, 'param-bsn-delete-btn');
      const events = wrapper.emitted('delete');
      expect(firstPayload(events)).toEqual({ section: 'parameter', index: 0 });
    });

    it('emits delete with input index payload after confirming', async () => {
      const wrapper = mountEditable();
      await clickDeleteAndConfirm(wrapper, 'input-leeftijd-delete-btn');
      const events = wrapper.emitted('delete');
      expect(firstPayload(events)).toEqual({ section: 'input', index: 0 });
    });

    it('emits delete with output index payload after confirming', async () => {
      const wrapper = mountEditable();
      await clickDeleteAndConfirm(wrapper, 'output-heeft_recht-delete-btn');
      const events = wrapper.emitted('delete');
      expect(firstPayload(events)).toEqual({ section: 'output', index: 0 });
    });

    it('emits delete with action index payload after confirming', async () => {
      const wrapper = mountEditable();
      await clickDeleteAndConfirm(wrapper, 'action-hoogte-delete-btn');
      const events = wrapper.emitted('delete');
      expect(firstPayload(events)).toEqual({ section: 'action', index: 0 });
    });
  });
});
