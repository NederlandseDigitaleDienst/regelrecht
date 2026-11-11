import { describe, it, expect, beforeAll } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import ArticleTextEditor from './ArticleTextEditor.vue';

// Vite is configured with `isCustomElement: tag => tag.startsWith('nldd-')`
// (vite.config.js), so Vue treats <nldd-*> tags as raw HTML elements. The
// real nldd-text-editor does not construct under happy-dom, so a minimal
// stand-in carries the parts of its API this component touches: a `value`
// property (Vue binds `:value` as a property once the element has one) and
// `clearHistory()`, which records that it was called.
class TextEditorStub extends HTMLElement {
  value = '';
  historyCleared = 0;
  clearHistory() {
    this.historyCleared += 1;
  }
}

beforeAll(() => {
  if (!customElements.get('nldd-text-editor')) {
    customElements.define('nldd-text-editor', TextEditorStub);
  }
});

/** The stand-in nldd-text-editor the component rendered. */
function editorElement(wrapper: ReturnType<typeof mountEditor>): TextEditorStub {
  const el = wrapper.element.querySelector('nldd-text-editor');
  if (!(el instanceof TextEditorStub)) throw new Error('no nldd-text-editor rendered');
  return el;
}

/** A state event as nldd-text-editor dispatches it, with every flag off except those given. */
function stateEvent(overrides: { canUndo?: boolean } = {}) {
  return new CustomEvent('nldd-text-editor-state', {
    detail: {
      active: { bold: false, italic: false, bulletList: false, orderedList: false },
      canIndent: false,
      canOutdent: false,
      canUndo: false,
      canRedo: false,
      empty: true,
      ...overrides,
    },
  });
}

function mountEditor(props = {}) {
  return mount(ArticleTextEditor, {
    props: {
      article: { number: '1', text: '' },
      editable: true,
      modelValue: '',
      ...props,
    },
  });
}

describe('ArticleTextEditor', () => {
  it('mounts cleanly with markdown modelValue and exposes format toggles', () => {
    const wrapper = mountEditor({ modelValue: '**hello**' });

    // The toolbar lives in the parent pane-header now; this component
    // only needs to expose its format helpers so the parent can drive
    // them. Verify the contract used by EditorApp.vue's pane-header.
    const exposed = wrapper.vm;
    expect(typeof exposed.toggleBold).toBe('function');
    expect(typeof exposed.toggleItalic).toBe('function');
    expect(typeof exposed.toggleBulletList).toBe('function');
    expect(typeof exposed.toggleOrderedList).toBe('function');
    // activeFormats reflects the editor's current selection - only the shape
    // is part of the contract with the parent toolbar.
    expect(Object.keys(exposed.activeFormats).sort()).toEqual([
      'bold', 'bulletList', 'canIndent', 'canOutdent', 'italic', 'orderedList',
    ]);
  });

  it('shows the empty state when no article is selected', () => {
    const wrapper = mount(ArticleTextEditor, {
      props: { article: null, editable: true, modelValue: '' },
    });
    const empty = wrapper.find('nldd-inline-dialog');
    expect(empty.exists()).toBe(true);
    expect(empty.attributes('text')).toContain('Geen artikel geselecteerd');
  });

  it('emits update:modelValue with markdown when editor content changes', async () => {
    const wrapper = mountEditor({ modelValue: 'start' });
    const el = editorElement(wrapper);

    // The element sets `value` to the new Markdown before dispatching `input`.
    el.value = 'start **world**';
    el.dispatchEvent(new Event('input'));
    await nextTick();

    expect(wrapper.emitted('update:modelValue')).toEqual([['start **world**']]);
  });

  it('updates the editor content when modelValue changes externally', async () => {
    const wrapper = mountEditor({ modelValue: 'first' });
    const el = editorElement(wrapper);
    expect(el.value).toBe('first');

    await wrapper.setProps({ modelValue: 'second paragraph' });

    expect(el.value).toBe('second paragraph');
  });

  it('clearHistory empties the undo stack so a discard cannot be stepped back into', async () => {
    const wrapper = mountEditor({ modelValue: 'start' });
    const el = editorElement(wrapper);

    // An edit makes the history non-empty: the editor reports undo available.
    el.dispatchEvent(stateEvent({ canUndo: true }));
    await nextTick();
    expect(wrapper.vm.canUndo).toBe(true);

    // clearHistory hands the reset to the editor, which empties its history
    // and reports that nothing is left to undo.
    wrapper.vm.clearHistory();
    expect(el.historyCleared).toBe(1);
    el.dispatchEvent(stateEvent({ canUndo: false }));
    await nextTick();
    expect(wrapper.vm.canUndo).toBe(false);
  });
});

// The editor renumbers ordered lists when it takes in a text, and an article's
// numbered paragraphs each read as a list of one: "2." comes back as "1.".
describe('ArticleTextEditor renumbering echo', () => {
  const ARTICLE = '1. Eerste lid.\n\n2. Tweede lid.';

  function typeInto(wrapper: ReturnType<typeof mountEditor>, value: string) {
    const el = editorElement(wrapper);
    el.value = value;
    el.dispatchEvent(new CustomEvent('input', { detail: { value } }));
  }

  it('drops an input that only renumbers the paragraphs', () => {
    const wrapper = mountEditor({ modelValue: ARTICLE });
    typeInto(wrapper, '1. Eerste lid.\n\n1. Tweede lid.');
    expect(wrapper.emitted('update:modelValue')).toBeUndefined();
  });

  it('passes on a real edit, renumbered or not', () => {
    const wrapper = mountEditor({ modelValue: ARTICLE });
    typeInto(wrapper, '1. Eerste lid, gewijzigd.\n\n1. Tweede lid.');
    typeInto(wrapper, '1. Eerste lid.\n\n2. Tweede lid!');
    expect(wrapper.emitted('update:modelValue')).toEqual([
      ['1. Eerste lid, gewijzigd.\n\n1. Tweede lid.'],
      ['1. Eerste lid.\n\n2. Tweede lid!'],
    ]);
  });
});
