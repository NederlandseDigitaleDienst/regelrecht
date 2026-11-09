// A small Wadler-style pretty printer. A rule is built as a document of
// tokens, spaces and soft line breaks; a group stays on one line when it fits
// and otherwise breaks every soft line in it. Layout never changes the token
// sequence: parentheses carry the structure, indentation only shows it.

import type { Doc, Segment, Token, TokenKind } from './types';

export const WIDTH = 160;
const TAB_WIDTH = 4;

export const line = Object.freeze({ kind: 'line' } as const);
// A line that always breaks, and so breaks every group around it: for forms
// that are a list on paper (an enumeration, a legend), whatever the width.
export const hardline = Object.freeze({ kind: 'hardline' } as const);
export const group = (...doc: Doc[]): Doc => ({ kind: 'group', doc });
export const nest = (...doc: Doc[]): Doc => ({ kind: 'nest', doc });
// A group that always breaks, whatever the width; it breaks the groups around it too.
export const brokenGroup = (...doc: Doc[]): Doc => ({ kind: 'group', doc, force: true });

/** A token: `tk` is keyword, term, name, literal, punct, fold, hole or error. */
export const tok = (tk: TokenKind, text: string, extra: Partial<Omit<Token, 'kind' | 'tk' | 'text'>> = {}): Token => ({ kind: 'tok', tk, text, ...extra });

function flatWidth(doc: Doc): number {
  if (typeof doc === 'string') return [...doc].length;
  if (Array.isArray(doc)) return doc.reduce((n, d) => n + flatWidth(d), 0);
  if (doc.kind === 'tok') return [...doc.text].length;
  if (doc.kind === 'line') return 1;
  if (doc.kind === 'hardline') return Infinity;
  if (doc.kind === 'group' && doc.force) return Infinity;
  return flatWidth(doc.doc);
}

/**
 * Lays out `doc`; returns a list of segments: tokens and whitespace strings.
 * Concatenating their `text` gives the reading.
 */
export function layout(doc: Doc, width = WIDTH): Segment[] {
  const out: Segment[] = [];
  let col = 0;
  // Stack of [indent, flat, doc]; processed last-in first-out.
  const stack: [number, boolean, Doc][] = [[0, false, doc]];
  const rest = () => {
    // Width the rest of the current line needs once flat: up to the first
    // breaking line in the remaining stack. A conservative approximation is
    // enough here: it only decides whether a group breaks.
    let n = 0;
    for (let i = stack.length - 1; i >= 0; i--) {
      const [, flat, d] = stack[i]!;
      const w = untilBreak(d, flat);
      n += w.n;
      if (w.stop) break;
    }
    return n;
  };
  // The `!`s below index within bounds the loops just checked.
  while (stack.length) {
    const [ind, flat, d] = stack.pop()!;
    if (typeof d === 'string') {
      out.push(d);
      col += [...d].length;
    } else if (Array.isArray(d)) {
      for (let i = d.length - 1; i >= 0; i--) stack.push([ind, flat, d[i]!]);
    } else if (d.kind === 'tok') {
      out.push(d);
      col += [...d.text].length;
    } else if (d.kind === 'line' || d.kind === 'hardline') {
      if (flat && d.kind === 'line') {
        out.push(' ');
        col += 1;
      } else {
        out.push('\n' + '\t'.repeat(ind));
        col = ind * TAB_WIDTH;
      }
    } else if (d.kind === 'nest') {
      stack.push([ind + 1, flat, d.doc]);
    } else if (d.kind === 'group') {
      if (d.force) {
        stack.push([ind, false, d.doc]);
      } else if (flat) {
        stack.push([ind, true, d.doc]);
      } else {
        const fits = col + flatWidth(d.doc) + rest() <= width;
        stack.push([ind, fits, d.doc]);
      }
    }
  }
  return out;
}

function untilBreak(doc: Doc, flat: boolean): { n: number; stop: boolean } {
  if (typeof doc === 'string') return { n: [...doc].length, stop: false };
  if (Array.isArray(doc)) {
    let n = 0;
    for (const d of doc) {
      const w = untilBreak(d, flat);
      n += w.n;
      if (w.stop) return { n, stop: true };
    }
    return { n, stop: false };
  }
  if (doc.kind === 'tok') return { n: [...doc.text].length, stop: false };
  if (doc.kind === 'line') return flat ? { n: 1, stop: false } : { n: 0, stop: true };
  if (doc.kind === 'hardline') return { n: 0, stop: true };
  if (doc.kind === 'group') return untilBreak(doc.doc, !doc.force);
  return untilBreak(doc.doc, flat);
}

export function segmentsToText(segments: readonly Segment[]): string {
  return segments.map((s) => (typeof s === 'string' ? s : s.text)).join('');
}
