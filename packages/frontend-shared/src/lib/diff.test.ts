import { describe, it, expect } from 'vitest';
import { lineDiff, compactDiff, definitionDiff, articleTextDiff } from './diff';

describe('lineDiff', () => {
  it('marks an added line and keeps the rest as context', () => {
    expect(lineDiff('a\nb', 'a\nx\nb')).toEqual([
      { type: 'context', text: 'a' },
      { type: 'add', text: 'x' },
      { type: 'context', text: 'b' },
    ]);
  });

  it('marks a removed line', () => {
    expect(lineDiff('a\nx\nb', 'a\nb')).toEqual([
      { type: 'context', text: 'a' },
      { type: 'del', text: 'x' },
      { type: 'context', text: 'b' },
    ]);
  });

  it('reports a changed line as a removal plus an addition', () => {
    expect(lineDiff('value: 1', 'value: 2')).toEqual([
      { type: 'del', text: 'value: 1' },
      { type: 'add', text: 'value: 2' },
    ]);
  });

  it('reports nothing but context for identical text', () => {
    expect(lineDiff('a\nb', 'a\nb').every((d) => d.type === 'context')).toBe(true);
  });
});

describe('compactDiff', () => {
  it('collapses runs of untouched lines into a single gap', () => {
    const a = ['1', '2', '3', '4', '5', '6', '7', '8', '9'].join('\n');
    const b = ['1', '2', '3', '4', 'X', '6', '7', '8', '9'].join('\n');

    const out = compactDiff(a, b);

    // The change plus two lines of context either side survives; the far ends
    // fold into one gap each.
    expect(out.filter((d) => d.type === 'gap')).toHaveLength(2);
    expect(out.filter((d) => d.type === 'del')).toEqual([{ type: 'del', text: '5' }]);
    expect(out.filter((d) => d.type === 'add')).toEqual([{ type: 'add', text: 'X' }]);
    expect(out.map((d) => d.text)).toEqual(['…', '3', '4', '5', 'X', '6', '7', '…']);
  });

  it('returns a single gap when nothing changed', () => {
    expect(compactDiff('a\nb\nc', 'a\nb\nc')).toEqual([{ type: 'gap', text: '…' }]);
  });
});

describe('articleTextDiff', () => {
  const doc = (text: unknown) => ({ articles: [{ number: '6.10', text }] });

  it('reports a changed law text with its article number', () => {
    expect(articleTextDiff(doc('Vier procent.'), doc('Vijf procent.'))).toEqual([
      { article: '6.10', before: 'Vier procent.', after: 'Vijf procent.' },
    ]);
  });

  it('reports nothing when the text stays the same', () => {
    expect(articleTextDiff(doc('Vier procent.'), doc('Vier procent.'))).toEqual([]);
  });

  // An article without law text is not the same as an empty law text; there
  // is nothing to say about it, so the diff says nothing.
  it('skips an article without law text', () => {
    expect(articleTextDiff(doc(undefined), doc('Nu wel tekst.'))).toEqual([]);
    expect(articleTextDiff(doc('Was er tekst.'), doc(undefined))).toEqual([]);
  });

  it('skips an article that no longer exists in the working version', () => {
    expect(articleTextDiff(doc('Tekst.'), { articles: [] })).toEqual([]);
  });
});

describe('definitionDiff', () => {
  const doc = (value: unknown) => ({
    articles: [{ number: '6.10', machine_readable: { definitions: { voet_ratio: { value } } } }],
  });

  it('reports a changed definition value with article and name', () => {
    expect(definitionDiff(doc(0.84), doc(0.9))).toEqual([
      { article: '6.10', name: 'voet_ratio', before: 0.84, after: 0.9 },
    ]);
  });

  it('reports nothing when the value is unchanged', () => {
    expect(definitionDiff(doc(0.84), doc(0.84))).toEqual([]);
  });

  it('matches articles by number, not by position', () => {
    const base = {
      articles: [
        { number: '1.1', machine_readable: { definitions: { a: { value: 1 } } } },
        { number: '2.2', machine_readable: { definitions: { b: { value: 2 } } } },
      ],
    };
    const current = {
      articles: [
        { number: '2.2', machine_readable: { definitions: { b: { value: 99 } } } },
        { number: '1.1', machine_readable: { definitions: { a: { value: 1 } } } },
      ],
    };

    expect(definitionDiff(base, current)).toEqual([
      { article: '2.2', name: 'b', before: 2, after: 99 },
    ]);
  });

  // The sheet shows value changes, not structural ones: a definition the
  // working version dropped has no new value to put next to the old one, so it is
  // deliberately absent here rather than rendered as a change to undefined.
  it('ignores a definition that the current document no longer has', () => {
    const current = { articles: [{ number: '6.10', machine_readable: { definitions: {} } }] };
    expect(definitionDiff(doc(0.84), current)).toEqual([]);
  });

  it('survives a document without articles or machine_readable', () => {
    expect(definitionDiff(null, null)).toEqual([]);
    expect(definitionDiff({ articles: [{ number: '1' }] }, { articles: [{ number: '1' }] })).toEqual([]);
  });
});
