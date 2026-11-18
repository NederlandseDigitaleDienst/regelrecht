import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import DocumentList from './DocumentList.vue';

// DS elements (nldd-list, nldd-list-item, nldd-text-cell, …) compile to raw
// custom elements under happy-dom; we assert against attributes/markup rather
// than rendered shadow DOM.
const DOCS = [{ path: 'beleid.md' }, { path: 'nota/bijlage.txt' }];

function mountList(props: { selectedPath?: string } = {}) {
  return mount(DocumentList, {
    props: { documents: DOCS, ...props },
  });
}

describe('DocumentList', () => {
  it('renders one row per document (no new-document row)', () => {
    const wrapper = mountList();
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(DOCS.length);
  });

  it('hides the .md extension but keeps .txt visible', () => {
    const wrapper = mountList();
    const texts = wrapper.findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(texts).toContain('beleid');
    expect(texts).toContain('nota/bijlage.txt');
  });

  it('emits select with the document path when a row is clicked', async () => {
    const wrapper = mountList();
    const first = wrapper.findAll('nldd-list-item')[0];
    expect(first).toBeDefined();
    await first!.trigger('click');
    const emitted = wrapper.emitted('select');
    expect(emitted).toBeTruthy();
    expect(emitted![0]).toEqual(['beleid.md']);
  });

  it('marks the open document row as current', () => {
    const wrapper = mountList({ selectedPath: 'beleid.md' });
    const current = wrapper
      .findAll('nldd-list-item')
      .filter((i) => i.attributes('current') !== undefined);
    expect(current).toHaveLength(1);
  });

  it('shows a chevron trailing icon (in-place select)', () => {
    const wrapper = mountList();
    const icons = wrapper.findAll('nldd-icon').map((i) => i.attributes('icon'));
    expect(icons).toContain('chevron-right');
  });
});
