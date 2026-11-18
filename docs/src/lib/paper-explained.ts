/*
 * What the plain-language page takes from the paper itself.
 *
 * The page at /research/rules-as-executed/uitgelegd (and its English twin)
 * explains the position paper as published. Wherever it shows something the
 * paper shows (Figure 1, Figure 3, Figure 5, Table 1) it shows the paper's
 * own copy, read out of the generated paper HTML at build time, and every
 * "read this in the paper" link is checked against the paper's outline.
 *
 * The paper is frozen (AGENTS.md, "Published papers are frozen"), so nothing
 * here can drift: what this module returns is exactly what the paper says. The
 * failure it guards against is the other direction, a regenerated paper in
 * which a figure or a section moved. Then this throws at build time, rather
 * than the page quietly linking to an anchor that no longer exists or showing
 * a figure the paper no longer has.
 */
import paperHtml from '~/research/rules-as-executed.html?raw';
import headings from '~/research/rules-as-executed.headings.json';
import meta from '~/research/rules-as-executed.meta.json';

export { meta };

export const PAPER_PATH = '/research/rules-as-executed';

export interface SectionRef {
  href: string;
  /** The heading as the paper numbers it, e.g. "3.2 The Gap Between ...". */
  label: string;
}

/**
 * A link to a section of the paper, by the anchor the paper's own outline
 * carries. An anchor that is not in the outline is a build error.
 */
export function section(slug: string): SectionRef {
  const h = headings.find((x) => x.slug === slug);
  if (!h) throw new Error(`paper-explained: no section "${slug}" in the paper outline`);
  return { href: `${PAPER_PATH}#${encodeURIComponent(slug)}`, label: h.text };
}

/** A link to a figure or table of the paper, by its anchor. */
export function figureHref(id: string): string {
  if (!paperHtml.includes(`<figure id="${id}">`)) {
    throw new Error(`paper-explained: no figure "${id}" in the paper`);
  }
  return `${PAPER_PATH}#${encodeURIComponent(id)}`;
}

const ENTITIES: Record<string, string> = {
  ndash: '–',
  amp: '&',
  lt: '<',
  gt: '>',
  quot: '"',
  '#39': "'",
};

/**
 * Plain text from a fragment of the paper HTML, for rendering as text.
 *
 * Entities are decoded in one pass, so `&amp;lt;` stays the literal `&lt;`
 * rather than becoming `<`; tags are stripped after that, repeatedly, until
 * none is left. What comes out is only ever rendered as escaped text.
 */
function plain(html: string): string {
  let text = html.replace(/&(ndash|amp|lt|gt|quot|#39);/g, (_m, name: string) => ENTITIES[name]);
  let previous: string;
  do {
    previous = text;
    text = text.replace(/<[^>]*>/g, '');
  } while (text !== previous);
  return text.replace(/\s+/g, ' ').trim();
}

function figureBody(id: string): string {
  const open = `<figure id="${id}">`;
  const start = paperHtml.indexOf(open);
  if (start === -1) throw new Error(`paper-explained: no figure "${id}" in the paper`);
  const end = paperHtml.indexOf('</figure>', start);
  if (end === -1) throw new Error(`paper-explained: figure "${id}" is not closed`);
  return paperHtml.slice(start + open.length, end);
}

export interface CodeFigure {
  /** The code as the paper prints it, still HTML-escaped (render with set:html). */
  codeHtml: string;
  /** The caption as plain text, including its "Figure N:" label. */
  caption: string;
  href: string;
}

/** A figure whose body is a code listing: Figures 1, 3 and 5. */
export function codeFigure(id: string): CodeFigure {
  const body = figureBody(id);
  const code = body.match(/<nldd-code-viewer[^>]*>([\s\S]*?)<\/nldd-code-viewer>/);
  if (!code) throw new Error(`paper-explained: figure "${id}" has no code listing`);
  const caption = body.match(/<figcaption[^>]*>([\s\S]*?)<\/figcaption>/);
  if (!caption) throw new Error(`paper-explained: figure "${id}" has no caption`);
  return { codeHtml: code[1], caption: plain(caption[1]), href: figureHref(id) };
}

export type Mark = 'yes' | 'part' | 'no';

export interface LandscapeRow {
  approach: string;
  exec: Mark;
  publ: Mark;
  bound: Mark;
  /** The row the paper sets apart with a rule: the proposal itself. */
  isProposal: boolean;
}

export interface Landscape {
  caption: string;
  rows: LandscapeRow[];
  href: string;
}

/** A cell of Table 1. Anything but the three values the paper uses is a build error. */
function mark(cell: string): Mark {
  if (cell === 'yes') return 'yes';
  if (cell === 'part') return 'part';
  if (cell === '\u2013') return 'no';
  throw new Error(`paper-explained: unexpected Table 1 cell "${cell}"`);
}

/** Table 1, row for row as the paper has it. */
export function landscape(): Landscape {
  const id = 'tab:landscape';
  const body = figureBody(id);
  const caption = body.match(/<figcaption[^>]*>([\s\S]*?)<\/figcaption>/);
  if (!caption) throw new Error('paper-explained: Table 1 has no caption');
  const tbody = body.match(/<tbody>([\s\S]*?)<\/tbody>/);
  if (!tbody) throw new Error('paper-explained: Table 1 has no body');

  const rows: LandscapeRow[] = [];
  for (const tr of tbody[1].matchAll(/<tr([^>]*)>([\s\S]*?)<\/tr>/g)) {
    const name = tr[2].match(/<th[^>]*>([\s\S]*?)<\/th>/);
    const cells = [...tr[2].matchAll(/<td[^>]*>([\s\S]*?)<\/td>/g)].map((m) => plain(m[1]));
    if (!name || cells.length !== 3) {
      throw new Error('paper-explained: a row of Table 1 is not in the expected shape');
    }
    rows.push({
      approach: plain(name[1]),
      exec: mark(cells[0]),
      publ: mark(cells[1]),
      bound: mark(cells[2]),
      isProposal: tr[1].includes('rr-rule'),
    });
  }
  if (rows.length === 0) throw new Error('paper-explained: Table 1 has no rows');
  return { caption: plain(caption[1]), rows, href: figureHref(id) };
}
