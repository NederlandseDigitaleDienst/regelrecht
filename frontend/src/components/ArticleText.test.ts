// @vitest-environment jsdom
//
// This file asserts the exact DOM structure produced by the marked +
// DOMPurify article pipeline (e.g. that "1. " renders as <ol><li>). happy-dom
// 20.x (our default test environment) has a NodeIterator bug that DOMPurify
// >= 3.4.8 trips over while scrubbing: it strips the <ol>/<ul> wrapper and
// keeps only the <li>, so `querySelector('ol li')` returns null. Verified in
// real Chromium, and under jsdom, that the same DOMPurify output keeps the
// list intact, so this is purely a happy-dom quirk, not a production bug. Pin
// this file to jsdom (a spec-faithful NodeIterator) until happy-dom fixes it.
//
// TODO(happy-dom NodeIterator): once a happy-dom release sanitizes DOMPurify's
// <ol>/<ul> output without stripping the wrapper, drop this docblock so the
// file returns to the default happy-dom environment. Re-check by temporarily
// removing the line and running this file's "ol li" assertions.
import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import ArticleText from './ArticleText.vue';

// ArticleText is the read-only Tekst pane (editor without write access, and
// the library reading view). These tests pin the markdown pipeline it renders
// through (useArticleMarkdown): lid prefixes become real lists, and harvested
// HTML never reaches the DOM unsanitized.

// nldd-* tags are compiled as custom elements (vite.config isCustomElement),
// so they render as-is and are asserted on by tag name, not stubbed.
function mountWith(article: { number: string; text: string } | null, extraProps: Record<string, unknown> = {}) {
  return mount(ArticleText, { props: { article, ...extraProps } });
}

describe('ArticleText markdown rendering', () => {
  it('renders a numbered lid prefix ("1. ") as an <ol><li>', () => {
    const wrapper = mountWith({
      number: '2',
      text: '1. een verzekerde heeft aanspraak op zorgtoeslag',
    });
    const li = wrapper.element.querySelector('ol li');
    expect(li).toBeTruthy();
    // The "1. " prefix is list markup, not text: the DOM text starts at "een".
    expect(li.textContent.trim()).toBe('een verzekerde heeft aanspraak op zorgtoeslag');
  });

  it('renders double newlines as separate leden (two list items)', () => {
    const wrapper = mountWith({
      number: '2',
      text: '1. eerste lid hier\n\n2. tweede lid daar',
    });
    const items = wrapper.element.querySelectorAll('ol li');
    expect(items).toHaveLength(2);
    expect(items[0].textContent.trim()).toBe('eerste lid hier');
    expect(items[1].textContent.trim()).toBe('tweede lid daar');
  });

  it('sanitizes embedded HTML before it reaches the DOM', () => {
    // Harvested law text could in principle carry arbitrary HTML; DOMPurify
    // must strip active content while keeping the visible text.
    const wrapper = mountWith({
      number: '1',
      text: 'tekst <img src=x onerror="hacked()"> met <script>hacked()</script>meer',
    });
    expect(wrapper.element.querySelector('script')).toBeNull();
    expect(wrapper.element.innerHTML).not.toContain('onerror');
    expect(wrapper.element.textContent).toContain('tekst');
    expect(wrapper.element.textContent).toContain('meer');
  });

  it('raw mode renders plain paragraphs without markdown parsing', () => {
    const wrapper = mountWith(
      { number: '1', text: '1. eerste lid\n\n2. tweede lid' },
      { raw: true },
    );
    // No list: each "\n\n"-separated block becomes a literal <p>.
    expect(wrapper.element.querySelector('ol')).toBeNull();
    const ps = wrapper.element.querySelectorAll('p');
    expect(ps).toHaveLength(2);
    expect(ps[0].textContent).toBe('1. eerste lid');
    expect(ps[1].textContent).toBe('2. tweede lid');
  });

  it('shows the empty state when no article is selected', () => {
    const wrapper = mountWith(null);
    expect(wrapper.find('nldd-inline-dialog').exists()).toBe(true);
    expect(wrapper.find('nldd-rich-text').exists()).toBe(false);
  });
});
