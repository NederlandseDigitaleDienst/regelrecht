// The shapes RegelRijm works with. An action tree comes from YAML that may be
// half-edited, so a node is `unknown` until a guard (`isOperation`,
// `isReference`) says more; nothing here claims the tree is valid.

/** An operation node: `operation` plus whatever fields the YAML holds. */
export interface Operation {
  operation: string;
  [field: string]: unknown;
}

/** One step of a path into a tree: a field name or a list index. */
export type PathStep = string | number;
export type Path = readonly PathStep[];

/** A unit a literal can take from its position. */
export type Unit = 'euro' | 'eurocent' | 'ratio' | 'percentage' | 'years' | 'weeks' | 'months' | 'days';

/** The kinds of token a reading consists of. */
export type TokenKind = 'keyword' | 'term' | 'name' | 'literal' | 'punct' | 'fold' | 'hole' | 'error' | 'letter';

/** An input a folded part names in its signature. */
export interface FoldInput {
  ref: string;
  tk: TokenKind;
  text: string;
}

export interface Token {
  kind: 'tok';
  tk: TokenKind;
  text: string;
  /** Where in the normalised action the token comes from. */
  path?: Path;
  /** The reference a name or term reads (`$a.b`). */
  ref?: string;
  /** The segment of that reference this token shows (`b`). */
  segment?: string;
  letter?: string;
  number?: string;
  inputs?: FoldInput[];
}

/** A piece of a laid-out reading: a token, or whitespace. */
export type Segment = string | Token;

/** A document for the pretty printer (see doc.ts). */
export type Doc =
  | string
  | Token
  | { readonly kind: 'line' }
  | { readonly kind: 'hardline' }
  | { kind: 'group' | 'nest'; doc: Doc[]; force?: boolean }
  | Doc[];

/** A name → term map that makes a reading use a corpus's own words. */
export type Glossary = Record<string, unknown>;

export interface Vocabulary {
  termOf(name: string): string | null;
  nameOf(term: string): string | null;
}

/** What a reading needs from the article: declared units, vocabulary, constants. */
export interface ArticleContext {
  declared: Map<string, Unit>;
  vocab: Vocabulary;
  /** The article's definitions; a hand-built context may leave them out. */
  constants?: ReadonlySet<string>;
}

/** An action as `{ output, value }` after normalisation. */
export interface NormalisedAction {
  output?: unknown;
  value?: unknown;
  resolve?: unknown;
}

/** True for a plain object (not null, not a list, not a Date). */
export function isRecord(v: unknown): v is Record<string, unknown> {
  return v !== null && typeof v === 'object' && !Array.isArray(v) && !(v instanceof Date);
}
