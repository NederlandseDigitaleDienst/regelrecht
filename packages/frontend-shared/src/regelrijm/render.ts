// The forms of RegelRijm, one per operation (RFC-054). Each form returns a
// document and the level it reads at; an operand that must be tighter than
// what it got is parenthesised. That rule, and nothing in the layout, is what
// makes two different trees never read alike.
import { brokenGroup, group, nest, line, hardline, tok, layout, segmentsToText } from './doc';
import { formatLiteral, formatNumber, MONTHS, PERIOD_WORDS } from './literals';
import { humanizeName, referenceSegments } from './names';
import { isOperation, isReference, OPERATION_FIELDS } from './normalise';
import { exprUnit, pathKey } from './units';
import { valueKind, freeInputs, SIGNATURE_MAX_INPUTS } from './signature';
import type { Doc, FoldInput, Segment, Operation, Path, Unit, Vocabulary } from './types';

/** A rendered part: its document and the level it reads at. */
export interface Rendered {
  doc: Doc;
  level: number;
}

/** One operation as `record` collects it, for chooseLegend. */
export interface PartRecord {
  path: Path;
  v: Operation;
  num: string;
  depth: number;
  order: number;
  /** The one-line reading, set once the part is rendered. */
  key?: string;
}

export interface FoldOptions {
  depth: number;
  open?: ReadonlySet<string>;
}

/** A folded part, for the reader to open. */
export interface Fold {
  number: string;
  path: Path;
  operation: string;
  label: string;
  inputs: FoldInput[];
}

export interface RendererContext {
  vocab: Vocabulary;
  units?: Map<string, Unit>;
  declared?: ReadonlyMap<string, Unit>;
  opUnits?: Map<string, Unit | null>;
  constants?: ReadonlySet<string>;
  fold?: FoldOptions | null;
  record?: boolean;
  hoist?: Map<string, string> | null;
  reps?: Map<string, PartRecord> | null;
}

// Levels, loosest first. An operand needs at least the level its position
// asks for, or it gets parentheses.
/**
 * Where a comparison stands. After `als` and `waarvoor` Dutch has a
 * subordinate clause, with the verb at the end: `als [a] waar is`, against
 * `[a] is waar` in a main clause.
 */
export type Clause = 'main' | 'sub';

/** Comparison words in a main clause and in a subordinate one (before and after the value). */
const CMP_SUB_WORDS: Record<string, [string | null, string | null]> = {
  EQUALS: [null, 'is'],
  GREATER_THAN: ['meer is dan', null],
  LESS_THAN: ['minder is dan', null],
  GREATER_THAN_OR_EQUAL: ['ten minste', 'is'],
  LESS_THAN_OR_EQUAL: ['ten hoogste', 'is'],
};

export const LEVEL = Object.freeze({ CHOICE: 1, LOGIC: 2, NOT: 3, CMP: 4, ARITH: 5, CLOSED: 6 });

const ARITH_WORDS: Record<string, string> = { ADD: 'plus', SUBTRACT: 'min', MULTIPLY: 'maal', DIVIDE: 'gedeeld door' };
const LOGIC_WORDS: Record<string, string> = { AND: 'én', OR: 'óf' };
const CMP_WORDS: Record<string, string> = {
  EQUALS: 'is',
  GREATER_THAN: 'is meer dan',
  LESS_THAN: 'is minder dan',
  GREATER_THAN_OR_EQUAL: 'is ten minste',
  LESS_THAN_OR_EQUAL: 'is ten hoogste',
};
const ROUND_WORDS: Record<string, string> = { ROUND: 'afgerond', CEIL: 'naar boven afgerond', FLOOR: 'naar beneden afgerond' };
const AGGREGATE_WORDS: Record<string, string> = { ADD: 'de som van', MIN: 'de kleinste waarde van', MAX: 'de grootste waarde van', ONLY: 'de enige waarde van' };
const DIFF_WORDS: Record<string, string> = { days: 'dagen', months: 'maanden', years: 'jaren' };
const PART_WORDS: Record<string, string> = { year: 'het jaartal van', month: 'het maandnummer van', day: 'het dagnummer van' };
const START_WORDS: Record<string, string> = { year: 'de eerste dag van het jaar waarin', month: 'de eerste dag van de maand waarin' };
const DATE_ADD_KEYS = ['years', 'months', 'weeks', 'days'];

const kw = (text: string, path?: Path) => tok('keyword', text, path ? { path } : {});

// The opening parenthesis stays on the line of what it opens; the inside
// breaks, if it must, one level deeper.
/** True when a document contains a parenthesis. */
function hasBracket(doc: Doc): boolean {
  if (typeof doc === 'string') return false;
  if (Array.isArray(doc)) return doc.some(hasBracket);
  if (doc.kind === 'tok') return doc.tk === 'keyword' && doc.text === '(';
  return 'doc' in doc && hasBracket(doc.doc);
}

/** The parts of a document's outer group, so they break with the group around them. */
function ungroup(doc: Doc): Doc {
  return typeof doc === 'object' && !Array.isArray(doc) && doc.kind === 'group' ? doc.doc : doc;
}

// Parentheses around `inside`. When the group does not fit, the inside goes
// on its own lines one level deeper and the closing parenthesis returns to
// the margin of the opening one. Parentheses inside parentheses always break
// that way, and then each operand of the operation directly inside stands on
// a line of its own: nested brackets on one line are hard to follow.
function brackets(open: Doc, inside: Doc): Doc {
  if (hasBracket(inside)) return brokenGroup(open, nest(line, ungroup(inside)), line, kw(')'));
  return group(open, nest(line, inside), line, kw(')'));
}

function wrap(doc: Doc): Doc {
  return brackets(kw('('), doc);
}

/** True when a document always breaks (it holds an enumeration or legend). */
function hasHardline(doc: Doc): boolean {
  if (doc === hardline) return true;
  if (Array.isArray(doc)) return doc.some(hasHardline);
  return typeof doc === 'object' && 'doc' in doc && hasHardline(doc.doc);
}

// Enumerations (RFC-054, "Enumerations"): an én/óf of this many conditions or
// more reads as a list of bulleted conditions, as in RegelSpraak. The bullet
// says how deep the list is (`•`, `••`, …), so the next bullet of the same
// depth or shallower ends an item: no parentheses, no closing word.
export const ENUMERATION_MIN = 3;
const ENUMERATION_HEADS: Record<string, string> = {
  AND: 'aan alle volgende voorwaarden is voldaan:',
  OR: 'aan ten minste één van de volgende voorwaarden is voldaan:',
};

/** A, B, …, Z, AA, BB, …: the letters of a legend. */
export function legendLetter(i: number): string {
  return String.fromCharCode(65 + (i % 26)).repeat(1 + Math.floor(i / 26));
}

function at(min: number, r: Rendered): Doc {
  return r.level >= min ? r.doc : wrap(r.doc);
}

/** `( a; b; c )`, the delimited list of MIN, MAX, LIST, IN and the short n-ary forms. */
function listDoc(items: Doc[]): Doc {
  if (items.length === 0) return [kw('('), ' ', kw(')')];
  const body: Doc[] = [];
  items.forEach((it, i) => {
    if (i > 0) body.push(kw(';'), line);
    body.push(it);
  });
  return brackets(kw('('), body);
}

/** Joins parts with single spaces. */
function sp(...parts: (Doc | null | undefined)[]): Doc[] {
  const out: Doc[] = [];
  parts.filter((p) => p !== null && p !== undefined).forEach((p, i) => {
    if (i > 0) out.push(' ');
    out.push(p as Doc);
  });
  return out;
}

export class Renderer {
  vocab: Vocabulary;
  units: Map<string, Unit>;
  fold: FoldOptions | null;
  enumDepth: number;
  declared: ReadonlyMap<string, Unit>;
  opUnits: Map<string, Unit | null>;
  constants: ReadonlySet<string>;
  record: PartRecord[] | null;
  hoist: Map<string, string> | null;
  reps: Map<string, PartRecord> | null;
  letters: Map<string, string>;
  legend: { letter: string; rep: PartRecord }[];
  numbers: Map<string, string>;
  folds: Fold[];
  clause: Clause;

  /**
   * @param {object} ctx
   *   vocab: from makeVocabulary
   *   units: Map pathKey → unit (literalUnits)
   *   fold: optional { depth, open } – show `depth` levels of operations
   *         below the start and fold deeper ones into their signature;
   *         `open` holds the path keys of folded parts the reader opened,
   *         each showing one level more
   *   opUnits: Map pathKey → unit of the position of each operation
   *   constants: names of the article's definitions, not inputs
   */
  constructor(ctx: RendererContext) {
    this.vocab = ctx.vocab;
    this.units = ctx.units ?? new Map();
    this.fold = ctx.fold ?? null;
    this.enumDepth = 0; // enumerations around the part being read
    this.clause = 'main';
    this.declared = ctx.declared ?? new Map();
    this.opUnits = ctx.opUnits ?? new Map();
    this.constants = ctx.constants ?? new Set();
    // Legend (RFC-054, "Legend"): `record` collects the flat text of every
    // operation for chooseLegend; `hoist` maps the path of each occurrence it
    // chose to its text, and `reps` that text to the occurrence that is read.
    this.record = ctx.record ? [] : null;
    this.hoist = ctx.hoist ?? null;
    this.reps = ctx.reps ?? null;
    this.letters = new Map(); // text → letter, in order of first use
    this.legend = []; // { letter, rep } per letter, in that same order
    this.numbers = new Map(); // pathKey → number, filled while rendering
    this.folds = [];
  }

  name(ref: string, path: Path): Rendered & { doc: Doc[] } {
    const segs = referenceSegments(ref);
    const parts: Doc[] = [];
    segs.forEach((seg, i) => {
      if (i > 0) parts.push(' ', kw('van'), ' ');
      const term = this.vocab.termOf(seg);
      parts.push(term
        ? tok('term', `‹${term}›`, { ref, segment: seg, path })
        : tok('name', `[${humanizeName(seg)}]`, { ref, segment: seg, path }));
    });
    return { doc: parts, level: LEVEL.CLOSED };
  }

  localName(name: string, path: Path): Doc {
    return this.name(`$${name}`, path).doc;
  }

  literal(v: unknown, path: Path): Rendered {
    const text = formatLiteral(v, this.units.get(pathKey(path)) ?? null);
    if (text === null) return { doc: tok('error', `⟨geen geldige waarde: ${JSON.stringify(v)}⟩`, { path }), level: LEVEL.CLOSED };
    return { doc: tok('literal', text, { path }), level: LEVEL.CLOSED };
  }

  hole(field: string, path: Path): Rendered {
    return { doc: tok('hole', `⟨ontbreekt: ${field}⟩`, { path }), level: LEVEL.CLOSED };
  }

  /** Renders a value at `path`; `num` is the number of the nearest operation. */
  node(v: unknown, path: Path, num: string, depth: number): Rendered {
    if (v === undefined) return this.hole(String(path[path.length - 1]), path);
    if (isReference(v)) return this.name(v, path);
    if (!isOperation(v)) {
      if (v !== null && typeof v === 'object' && !(v instanceof Date)) {
        return { doc: tok('error', '⟨geen operatie⟩', { path }), level: LEVEL.CLOSED };
      }
      return this.literal(v, path);
    }
    const pk = pathKey(path);
    this.numbers.set(pk, num);
    if (this.hoist?.has(pk)) {
      // `has` was checked just above.
      const key = this.hoist.get(pk)!;
      let letter = this.letters.get(key);
      if (!letter) {
        letter = legendLetter(this.letters.size);
        this.letters.set(key, letter);
        // chooseLegend puts every hoisted text in `reps`.
        this.legend.push({ letter, rep: this.reps!.get(key)! });
      }
      return { doc: tok('letter', letter, { path, letter }), level: LEVEL.CLOSED };
    }
    if (this.fold && depth >= this.fold.depth && this.fold.open?.has(pk)) {
      // Opened by the reader: this part in full, its own parts folded.
      const fold = this.fold;
      this.fold = { ...fold, depth: depth + 1 };
      const res = this.operation(v, path, num, depth);
      this.fold = fold;
      return res;
    }
    if (this.fold && depth >= this.fold.depth) {
      const { label, inputs } = this.signatureOf(v, path);
      // A part that reads shorter in full than its signature is not folded:
      // folding is there to save reading, and here it would cost.
      const fold = this.fold;
      this.fold = null;
      const full = this.operation(v, path, num, depth);
      this.fold = fold;
      const flat = segmentsToText(layout(full.doc, Infinity));
      if (!flat.includes('\n') && flat.length <= label.length + 2) return full;
      this.folds.push({ number: num, path, operation: v.operation, label, inputs });
      return { doc: tok('fold', `⟨${label}⟩`, { path, number: num, inputs }), level: LEVEL.CLOSED };
    }
    if (!this.record) return this.operation(v, path, num, depth);
    const rec: PartRecord = { path, v, num, depth, order: this.record.length };
    this.record.push(rec);
    const res = this.operation(v, path, num, depth);
    rec.key = segmentsToText(layout(res.doc, Infinity)).replace(/\s+/g, ' ');
    return res;
  }

  /**
   * What the operation at `path` gives and what it reads, like the signature
   * of a call: `bedrag uit [a] en [b]`. Derived from the tree and the article
   * only; the number of the part stays in the fold's metadata.
   */
  signature(v: Operation, path: Path): string {
    return this.signatureOf(v, path).label;
  }

  /** The signature with the inputs it reads, each as `{ ref, tk, text }`. */
  signatureOf(v: Operation, path: Path): { label: string; inputs: FoldInput[] } {
    const kind = valueKind(v, this.opUnits.get(pathKey(path)) ?? null, this.declared);
    const prep = kind === 'voorwaarde' ? 'over' : 'uit';
    const inputs = freeInputs(v, this.constants).map((ref) => {
      const doc = this.name(ref, path).doc;
      const only = doc.length === 1 ? doc[0] : undefined;
      const tk = only && typeof only === 'object' && 'tk' in only ? only.tk : 'name';
      return { ref, tk, text: segmentsToText(layout(doc, Infinity)) };
    });
    if (!inputs.length) return { label: `${kind} ${prep} vaste waarden`, inputs };
    const shown = inputs.slice(0, SIGNATURE_MAX_INPUTS).map((i) => i.text);
    const rest = inputs.length - shown.length;
    const list = rest > 0
      ? `${shown.join(', ')} en nog ${rest}`
      // With one input shown, `shown[0]` is that input.
      : shown.length === 1 ? shown[0]! : `${shown.slice(0, -1).join(', ')} en ${shown[shown.length - 1]}`;
    return { label: `${kind} ${prep} ${list}`, inputs };
  }

  /** The reading of a legend letter's definition: its occurrence, read in place. */
  definition(rep: PartRecord): Rendered {
    return this.operation(rep.v, rep.path, rep.num, rep.depth);
  }

  operation(n: Operation, path: Path, num: string, depth: number): Rendered {
    let k = 0;
    // Children are numbered in reading order; only operations take a number.
    const child = (v: unknown, sub: Path): Rendered => {
      const p = [...path, ...sub];
      if (isOperation(v)) {
        k += 1;
        return this.node(v, p, `${num}.${k}`, depth + 1);
      }
      return this.node(v, p, num, depth + 1);
    };
    const field = (key: string) => child(n[key], [key]);
    // Renders `fn` with the comparisons in it in clause `c`.
    const inClause = <T>(c: Clause, fn: () => T): T => {
      const prev = this.clause;
      this.clause = c;
      try {
        return fn();
      } finally {
        this.clause = prev;
      }
    };
    const sub = this.clause === 'sub';
    // `[s] woord [o] woord`, leaving out a word that is null.
    const cmp = (s: Doc, before: string | null, o: Doc, after: string | null): Doc =>
      group(...sp(s, before === null ? null : K(before), o, after === null ? null : K(after)));
    const op = n.operation;
    const K = (text: string) => kw(text, path);
    const list = (key: string, min: number = LEVEL.CHOICE) => {
      const items = n[key];
      return (Array.isArray(items) ? (items as unknown[]) : []).map((v, i) => at(min, child(v, [key, i])));
    };

    if (!OPERATION_FIELDS[op]) {
      return { doc: tok('error', `⟨onbekende operatie ${op}⟩`, { path }), level: LEVEL.CLOSED };
    }

    switch (op) {
      case 'ADD': case 'SUBTRACT': case 'MULTIPLY': case 'DIVIDE':
      case 'AND': case 'OR': {
        const key = op === 'AND' || op === 'OR' ? 'conditions' : 'values';
        // The cases above are exactly the keys of these two tables.
        const word = (ARITH_WORDS[op] ?? LOGIC_WORDS[op])!;
        if (!Array.isArray(n[key])) return { doc: sp(K(word), this.hole(key, [...path, key]).doc), level: LEVEL.CLOSED };
        const logic = key === 'conditions';
        if (n[key].length < 2) return { doc: sp(K(word), listDoc(list(key))), level: LEVEL.CLOSED };
        if (logic && n[key].length >= ENUMERATION_MIN) {
          this.enumDepth += 1;
          const bullet = '•'.repeat(this.enumDepth);
          // An item is delimited by the bullets around it, so it may itself
          // be an én/óf; only a choice (IF) still needs parentheses.
          // Each item is a sentence of its own: main clause order, also after `als`.
          const body = inClause('main', () => list(key, LEVEL.LOGIC)).flatMap((it) => [hardline, K(bullet), ' ', it]);
          this.enumDepth -= 1;
          // `logic` means AND or OR, the keys of ENUMERATION_HEADS.
          return { doc: [K(ENUMERATION_HEADS[op]!), nest(body)], level: LEVEL.LOGIC };
        }
        const items = list(key, logic ? LEVEL.NOT : LEVEL.CLOSED);
        // At least two items here (the length check above).
        const doc: Doc[] = [items[0]!];
        for (const it of items.slice(1)) doc.push(line, K(word), ' ', it);
        return { doc: group(doc), level: logic ? LEVEL.LOGIC : LEVEL.ARITH };
      }
      case 'MIN': case 'MAX': {
        const v = n.values;
        if (op === 'MAX' && Array.isArray(v) && v.length === 2 && v[0] === 0
          && isOperation(v[1]) && v[1].operation === 'SUBTRACT' && Array.isArray(v[1].values) && v[1].values.length === 2) {
          // MAX(0, a − b): the part of a above b. The 0 is the floor, not an operand.
          const sub = v[1].values;
          const a = at(LEVEL.CLOSED, child(sub[0], ['values', 1, 'values', 0]));
          const b = at(LEVEL.CLOSED, child(sub[1], ['values', 1, 'values', 1]));
          // Broken, both parts stand one level deeper, at the same indentation.
          return { doc: group(K('het deel van'), nest(line, a, line, K('boven'), ' ', b)), level: LEVEL.CLOSED };
        }
        const word = op === 'MIN' ? 'de kleinste van' : 'de grootste van';
        if (!Array.isArray(v)) return { doc: sp(K(word), this.hole('values', [...path, 'values']).doc), level: LEVEL.CLOSED };
        return { doc: sp(K(word), listDoc(list('values'))), level: LEVEL.CLOSED };
      }
      case 'NOT': {
        const inner = n.value;
        if (isOperation(inner) && inner.operation === 'EQUALS' && inner.subject !== undefined && inner.value !== undefined) {
          const ip = [...path, 'value'];
          k += 1;
          const inum = `${num}.${k}`;
          this.numbers.set(pathKey(ip), inum);
          let k2 = 0;
          const ichild = (v: unknown, key: string): Rendered => {
            if (isOperation(v)) {
              k2 += 1;
              return this.node(v, [...ip, key], `${inum}.${k2}`, depth + 2);
            }
            return this.node(v, [...ip, key], inum, depth + 2);
          };
          const s = at(LEVEL.ARITH, ichild(inner.subject, 'subject'));
          if (inner.value === null) return { doc: group(s, ' ', K(sub ? 'bekend is' : 'is bekend')), level: LEVEL.CMP };
          const o = at(LEVEL.ARITH, ichild(inner.value, 'value'));
          return { doc: sub ? cmp(s, 'niet', o, 'is') : cmp(s, 'is niet', o, null), level: LEVEL.CMP };
        }
        return { doc: sp(K('niet'), at(LEVEL.CLOSED, field('value'))), level: LEVEL.NOT };
      }
      case 'EQUALS': case 'GREATER_THAN': case 'LESS_THAN': case 'GREATER_THAN_OR_EQUAL': case 'LESS_THAN_OR_EQUAL': {
        const s = at(LEVEL.ARITH, field('subject'));
        if (op === 'EQUALS' && n.value === null) return { doc: group(s, ' ', K('ontbreekt')), level: LEVEL.CMP };
        const o = at(LEVEL.ARITH, field('value'));
        // The case lists exactly the keys of CMP_WORDS and CMP_SUB_WORDS.
        const [before, after] = sub ? CMP_SUB_WORDS[op]! : [CMP_WORDS[op]!, null];
        return { doc: cmp(s, before, o, after), level: LEVEL.CMP };
      }
      case 'IN': {
        const s = at(LEVEL.ARITH, field('subject'));
        if (Array.isArray(n.values)) {
          const vs = listDoc(list('values'));
          return { doc: sub ? cmp(s, 'een van', vs, 'is') : cmp(s, 'is een van', vs, null), level: LEVEL.CMP };
        }
        const v = at(LEVEL.CLOSED, field('value'));
        return { doc: sub ? cmp(s, 'in', v, 'staat') : cmp(s, 'staat in', v, null), level: LEVEL.CMP };
      }
      case 'IF': {
        if (!Array.isArray(n.cases)) return { doc: sp(K('als'), this.hole('cases', [...path, 'cases']).doc), level: LEVEL.CHOICE };
        // `waarde als voorwaarde, waarde als voorwaarde, anders waarde`: the
        // value first, then the condition it holds under, as a subordinate
        // clause. Broken, every case stands on a line of its own.
        const cases = n.cases;
        const parts: Doc[] = [];
        // After a bulleted condition the comma opens the next line, at the
        // margin of the value, so it does not read as one more item.
        let afterList = false;
        const separate = () => {
          if (afterList) parts.push(line, K(','), ' ');
          else parts.push(K(','), line);
        };
        if (cases.length === 0) parts.push(K('geen geval'));
        cases.forEach((c, i) => {
          // Rendered in reading order, so the numbers follow the text.
          const t = at(LEVEL.LOGIC, c && typeof c === 'object' && 'then' in c ? child(c.then, ['cases', i, 'then']) : this.hole('then', [...path, 'cases', i, 'then']));
          const w = at(LEVEL.LOGIC, c && typeof c === 'object' && 'when' in c
            ? inClause('sub', () => child(c.when, ['cases', i, 'when']))
            : this.hole('when', [...path, 'cases', i, 'when']));
          if (i > 0) separate();
          afterList = hasHardline(w);
          // A bulleted condition breaks anyway; `als` stays on the line of the value.
          parts.push(afterList ? [t, ' ', K('als'), ' ', w] : group(t, nest(line, K('als'), ' ', w)));
        });
        if ('default' in n) {
          if (cases.length) separate();
          else parts.push(K(','), ' ');
          parts.push(K('anders'), ' ', at(LEVEL.LOGIC, field('default')));
        }
        return { doc: group(parts), level: LEVEL.CHOICE };
      }
      case 'LIST':
        if (!Array.isArray(n.items)) return { doc: sp(K('de lijst'), this.hole('items', [...path, 'items']).doc), level: LEVEL.CLOSED };
        return { doc: sp(K('de lijst'), listDoc(list('items'))), level: LEVEL.CLOSED };
      case 'FOREACH': {
        // Operands are rendered in reading order, so fold numbers follow the text.
        const x = this.localName(String(n.as ?? 'item'), [...path, 'as']);
        const quantor = n.combine === 'AND' || n.combine === 'OR';
        const b0 = quantor ? null : at(LEVEL.CLOSED, field('body'));
        const c = at(LEVEL.CLOSED, field('collection'));
        const f: Doc[] = 'filter' in n ? [line, K('waarvoor'), ' ', at(LEVEL.CLOSED, inClause('sub', () => field('filter')))] : [];
        // Exactly one of the two is rendered, so `b` is always set.
        const b = (quantor ? at(LEVEL.CLOSED, field('body')) : b0)!;
        if (quantor) {
          const head = n.combine === 'AND' ? 'voor elke' : 'voor ten minste één';
          return { doc: group(K(head), ' ', x, ' ', K('in'), ' ', c, nest(...f, line, K('geldt'), ' ', b)), level: LEVEL.CLOSED };
        }
        const known = n.combine === undefined ? 'de lijst van' : AGGREGATE_WORDS[String(n.combine)];
        const head: Doc = known ? K(known) : [tok('error', `⟨onbekende combine ${n.combine}⟩`, { path }), ' ', K('van')];
        // Not closed: an optional `waarvoor` ends it, which would dangle when
        // it stood as the collection of another FOREACH.
        return { doc: group(head, ' ', b, nest(line, K('over elke'), ' ', x, ' ', K('in'), ' ', c, ...f)), level: LEVEL.ARITH };
      }
      case 'AGE':
        return {
          doc: group(K('het aantal volle jaren tussen'), ' ', at(LEVEL.CLOSED, field('date_of_birth')), ' ', K('en'), ' ', at(LEVEL.CLOSED, field('reference_date'))),
          level: LEVEL.CLOSED,
        };
      case 'DATE_ADD': {
        const keys = DATE_ADD_KEYS.filter((key) => key in n);
        if (keys.length === 0) return { doc: sp(K('geen termijn na'), at(LEVEL.CLOSED, field('date'))), level: LEVEL.CLOSED };
        const offsets: Doc[] = [];
        keys.forEach((key, i) => {
          if (i > 0) offsets.push(i === keys.length - 1 ? ' ' : '', i === keys.length - 1 ? K('en') : K(','), ' ');
          // `key` comes from DATE_ADD_KEYS, which are all keys of PERIOD_WORDS.
          const [one, many] = PERIOD_WORDS[key]!;
          offsets.push(at(LEVEL.CLOSED, child(n[key], [key])), ' ', K(n[key] === 1 ? one : many));
        });
        const d = at(LEVEL.CLOSED, field('date'));
        return { doc: group(...offsets, ' ', K('na'), ' ', d), level: LEVEL.ARITH };
      }
      case 'DATE': {
        const y = at(LEVEL.CLOSED, field('year'));
        const { month, day } = n;
        if (typeof month === 'number' && Number.isInteger(month) && month >= 1 && month <= 12
          && typeof day === 'number' && Number.isInteger(day) && day >= 1 && day <= 31) {
          // `month` is checked to be 1 to 12 just above.
          return { doc: sp(tok('literal', String(day), { path: [...path, 'day'] }), K(MONTHS[month - 1]!), K('van'), y), level: LEVEL.CLOSED };
        }
        const m = at(LEVEL.CLOSED, field('month'));
        const d = at(LEVEL.CLOSED, field('day'));
        return { doc: group(K('de datum met jaar'), ' ', y, K(','), ' ', K('maand'), ' ', m, ' ', K('en dag'), ' ', d), level: LEVEL.CLOSED };
      }
      case 'DAY_OF_WEEK':
        return { doc: sp(K('de weekdag van'), at(LEVEL.CLOSED, field('date'))), level: LEVEL.CLOSED };
      case 'DATE_DIFF': {
        const a = at(LEVEL.CLOSED, field('from'));
        const b = at(LEVEL.CLOSED, field('to'));
        const diff = DIFF_WORDS[String(n.in)];
        if (diff) return { doc: group(K(`het aantal ${diff} van`), ' ', a, ' ', K('tot'), ' ', b), level: LEVEL.CLOSED };
        if (isReference(n.in)) return { doc: group(K('het verschil van'), ' ', a, ' ', K('tot'), ' ', b, ' ', K('in'), ' ', this.name(n.in, [...path, 'in']).doc), level: LEVEL.CLOSED };
        return { doc: group(K('het verschil van'), ' ', a, ' ', K('tot'), ' ', b, ' ', K('in'), ' ', tok('error', `⟨onbekende eenheid ${JSON.stringify(n.in)}⟩`, { path: [...path, 'in'] })), level: LEVEL.CLOSED };
      }
      case 'DATE_PART': case 'START_OF': {
        const words = op === 'DATE_PART' ? PART_WORDS : START_WORDS;
        const word = words[String(n.in)];
        if (!word) return { doc: tok('error', `⟨onbekend onderdeel ${n.in}⟩`, { path }), level: LEVEL.CLOSED };
        const d = at(LEVEL.CLOSED, field('date'));
        return { doc: op === 'DATE_PART' ? sp(K(word), d) : sp(K(word), d, K('valt')), level: LEVEL.CLOSED };
      }
      case 'ROUND': case 'CEIL': case 'FLOOR': {
        const p = n.precision;
        let how;
        if (typeof p !== 'number' || !Number.isInteger(p)) {
          const v = at(LEVEL.CHOICE, field('value'));
          return { doc: group(K(`${ROUND_WORDS[op]} op`), ' ', this.hole('precision', [...path, 'precision']).doc, ' ', brackets(K('('), v)), level: LEVEL.CLOSED };
        }
        const unit = exprUnit(n.value, this.declared);
        if (p === 0 && unit === 'euro') how = "hele euro's";
        else if (p === 0 && unit === 'eurocent') how = 'hele centen';
        else if (p === -2 && unit === 'eurocent') how = "hele euro's";
        else if (p === 0) how = 'hele eenheden';
        else if (p === 1) how = '1 decimaal';
        else if (p > 1) how = `${formatNumber(p)} decimalen`;
        else how = `veelvouden van 1${'0'.repeat(-p)}`;
        const v = at(LEVEL.CHOICE, field('value'));
        return { doc: group(K(`${ROUND_WORDS[op]} op ${how}`), ' ', brackets(K('('), v)), level: LEVEL.CLOSED };
      }
      default:
        return { doc: tok('error', `⟨geen vorm voor ${op}⟩`, { path }), level: LEVEL.CLOSED };
    }
  }
}

// A repeated part is worth a letter only when it is this long (characters of
// its one-line reading); shorter ones read faster in place.
export const LEGEND_MIN = 40;

function isStrictAncestor(a: Path, b: Path): boolean {
  return a.length < b.length && a.every((s, i) => s === b[i]);
}

/**
 * Which repeated parts the reading names with a letter (RFC-054, "Legend").
 * Longest text first: a part is taken when it still occurs at least twice
 * outside the other occurrences of the parts already taken (inside the one
 * occurrence that is read, its definition, it still counts). Returns
 * `{ hoist: Map pathKey → text, reps: Map text → occurrence }`.
 */
export function chooseLegend(records: readonly PartRecord[], min = LEGEND_MIN): { hoist: Map<string, string>; reps: Map<string, PartRecord> } {
  // Every record has its key once rendering is done.
  const byKey = new Map<string, PartRecord[]>();
  for (const r of records) {
    const list = byKey.get(r.key!) ?? [];
    list.push(r);
    byKey.set(r.key!, list);
  }
  const keys = [...byKey.keys()]
    .filter((k) => k.length >= min && byKey.get(k)!.length >= 2)
    .sort((a, b) => b.length - a.length || byKey.get(a)![0]!.order - byKey.get(b)![0]!.order);
  const taken: PartRecord[][] = [];
  const visible = (r: PartRecord) => taken.every((t) => !t.slice(1).some((o) => isStrictAncestor(o.path, r.path)));
  for (const k of keys) {
    const occ = byKey.get(k)!.filter(visible);
    if (occ.length >= 2) taken.push(occ);
  }
  const hoist = new Map();
  const reps = new Map();
  for (const occ of taken) {
    // `taken` holds only lists of two or more keyed records.
    reps.set(occ[0]!.key!, occ[0]!);
    for (const o of occ) hoist.set(pathKey(o.path), o.key!);
  }
  return { hoist, reps };
}

/** Lays out a document into `{ segments, text }`. */
export function finish(doc: Doc): { segments: Segment[]; text: string } {
  const segments = layout(doc);
  return { segments, text: segmentsToText(segments) };
}
