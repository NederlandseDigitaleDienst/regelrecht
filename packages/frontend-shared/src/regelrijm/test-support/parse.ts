// Test-only parser for RegelRijm. It exists to prove the reading
// unambiguous: it returns EVERY parse of a text, and the round-trip test
// demands exactly one, equal to the normalised tree. It is not exported from
// the package and must never become an input channel (RFC-054).
import { MONTHS, PERIOD_WORDS } from '../literals';
import { isReference } from '../normalise';
import { exprUnit, literalUnits, pathKey } from '../units';
import { isRecord, type Operation, type Path, type Unit, type Vocabulary } from '../types';

type TokenType = 'term' | 'name' | 'str' | 'money' | 'date' | 'num' | 'p' | 'w';

interface LexToken {
  t: TokenType;
  m: RegExpExecArray;
}

/** One way to read from a position: the value and the position after it. */
type Result<T = unknown> = [T, number];
/** A grammar rule: every way to read from `pos`. */
type Rule<T = unknown> = (pos: number) => Result<T>[];

/** A number whose unit is not known yet (see `lit`). */
interface LitPlaceholder {
  __lit: 'money' | 'pct' | 'num' | 'qty';
  text: string;
  key?: string;
  plural?: boolean;
}

/** A precision that follows from the unit of the value (`hele euro's`). */
type PrecisionOf = (unit: Unit | null) => number | undefined;

/** A parsed action. */
export interface ParsedAction {
  output: string;
  value: unknown;
}

/** The capture groups of a token; the lexer only emits tokens whose regex has them. */
function group(tk: LexToken, i: number): string {
  return tk.m[i] ?? '';
}

const MONTH_RE = MONTHS.join('|');
const LEX: [TokenType, RegExp][] = [
  ['term', /‹([^›]*)›/y],
  ['name', /\[([^\]]*)\]/y],
  ['str', /“((?:\\.|[^”\\])*)”/y],
  ['money', /€ (-?\d{1,3}(?:\.\d{3})*,\d+)/y],
  ['date', new RegExp(`(\\d{1,2}) (${MONTH_RE}) (\\d{4})(?!\\d|,\\d|%)`, 'y')],
  ['num', /(-?\d+(?:,\d+)?)(%?)/y],
  ['p', /([();,])/y],
  ['w', /([^\s()[\]‹›“”;,]+)/y],
];

export function lex(text: string): LexToken[] {
  const out: LexToken[] = [];
  let i = 0;
  while (i < text.length) {
    // `i` stays below `text.length`.
    if (/\s/.test(text[i]!)) { i += 1; continue; }
    let hit = false;
    for (const [t, re] of LEX) {
      re.lastIndex = i;
      const m = re.exec(text);
      if (!m) continue;
      out.push({ t, m });
      i = re.lastIndex;
      hit = true;
      break;
    }
    if (!hit) throw new Error(`lex: unexpected character '${text[i]}' at ${i}`);
  }
  return out;
}

// A placeholder for a number whose unit the parser cannot know yet; it is
// resolved once the whole tree stands, with the same unit rule as the renderer.
const lit = (kind: LitPlaceholder['__lit'], text: string, extra: Partial<LitPlaceholder> = {}): LitPlaceholder => ({ __lit: kind, text, ...extra });

class Parser {
  t: LexToken[];
  vocab: Vocabulary;
  declared: ReadonlyMap<string, Unit>;
  memo: Map<string, Result[]>;

  constructor(tokens: LexToken[], vocab: Vocabulary, declared: ReadonlyMap<string, Unit>) {
    this.t = tokens;
    this.vocab = vocab;
    this.declared = declared;
    this.memo = new Map();
  }

  // Every rule: (pos) → array of [value, nextPos]. Memoised per rule and position.
  rule<T>(name: string, pos: number, fn: () => Result<T>[]): Result<T>[] {
    const key = `${name}@${pos}`;
    // A memo entry for this rule and position always holds this rule's results.
    const hit = this.memo.get(key) as Result<T>[] | undefined;
    if (hit) return hit;
    this.memo.set(key, []);
    const r = fn();
    this.memo.set(key, r);
    return r;
  }

  word(pos: number, phrase: string): number {
    const ws = phrase.split(' ');
    for (let i = 0; i < ws.length; i++) {
      const tk = this.t[pos + i];
      if (!tk) return -1;
      const v = tk.t === 'w' || tk.t === 'p' || tk.t === 'num' ? tk.m[0] : null;
      if (v !== ws[i]) return -1;
    }
    return pos + ws.length;
  }

  // seq helper: runs a sequence of steps, each step a function (pos) → [[v, pos]...] or a phrase.
  seq<T>(pos: number, steps: (string | Rule)[], build: (...vals: unknown[]) => T): Result<T>[] {
    let states: Result<unknown[]>[] = [[[], pos]];
    for (const step of steps) {
      const next: Result<unknown[]>[] = [];
      for (const [vals, p] of states) {
        if (typeof step === 'string') {
          const q = this.word(p, step);
          if (q >= 0) next.push([vals, q]);
        } else {
          for (const [v, q] of step(p)) next.push([[...vals, v], q]);
        }
      }
      states = next;
      if (!states.length) return [];
    }
    return states.map(([vals, p]) => [build(...vals), p]);
  }

  alt(...lists: Result[][]): Result[] {
    return lists.flat();
  }

  // Repetition `first (sep item)*` with at least `min` items in total.
  sepBy(pos: number, item: Rule, sep: string, min: number): Result<unknown[]>[] {
    const out: Result<unknown[]>[] = [];
    let states = item(pos).map(([v, p]): Result<unknown[]> => [[v], p]);
    while (states.length) {
      for (const [vals, p] of states) if (vals.length >= min) out.push([vals, p]);
      const next: Result<unknown[]>[] = [];
      for (const [vals, p] of states) {
        const q = this.word(p, sep);
        if (q < 0) continue;
        for (const [v, r] of item(q)) next.push([[...vals, v], r]);
      }
      states = next;
    }
    return out;
  }

  expr: Rule = (pos) => this.rule('expr', pos, () => this.alt(this.choice(pos), this.logic(pos)));

  // `waarde als voorwaarde, waarde als voorwaarde, anders waarde`.
  choice: Rule = (pos) => this.rule('choice', pos, () => {
    const out: Result[] = [];
    const caseAt = (p: number) => this.seq(p, [this.logic, 'als', this.logic], (t, w) => ({ when: w, then: t }));
    const anders = (p: number, cases: unknown[]): Result[] => {
      const res: Result[] = [[{ operation: 'IF', cases }, p]];
      const q = this.word(p, ', anders');
      if (q >= 0) for (const [d, r] of this.logic(q)) res.push([{ operation: 'IF', cases, default: d }, r]);
      return res;
    };
    const more = (p: number, cases: unknown[]): Result[] => {
      const res = anders(p, cases);
      const q = this.word(p, ',');
      if (q >= 0) for (const [c, r] of caseAt(q)) res.push(...more(r, [...cases, c]));
      return res;
    };
    for (const [c, p] of caseAt(pos)) out.push(...more(p, [c]));
    const g = this.word(pos, 'geen geval');
    if (g >= 0) out.push(...anders(g, []));
    return out;
  });

  logic: Rule = (pos) => this.rule('logic', pos, () => {
    const out: Result[] = [...this.operand(pos)];
    for (const [w, op] of [['én', 'AND'], ['óf', 'OR']]) {
      for (const [vals, p] of this.sepBy(pos, this.operand, w, 2)) out.push([{ operation: op, conditions: vals }, p]);
    }
    // Enumeration: `head • X • Y • Z`, at least three items. The first
    // bullet fixes the bullet of the list; the parser keeps no depth, it
    // only demands that every item of one list carries the same bullet.
    for (const [head, op] of [['aan alle volgende voorwaarden is voldaan:', 'AND'], ['aan ten minste één van de volgende voorwaarden is voldaan:', 'OR']]) {
      const q = this.word(pos, head);
      if (q < 0) continue;
      const tk = this.t[q];
      if (!tk || tk.t !== 'w' || !/^•+$/.test(group(tk, 1))) continue;
      const bullet = group(tk, 1);
      const item: Rule = (p) => {
        const r = this.word(p, bullet);
        return r < 0 ? [] : this.logic(r);
      };
      let states = item(q).map(([v, p]): Result<unknown[]> => [[v], p]);
      while (states.length) {
        const next: Result<unknown[]>[] = [];
        for (const [vals, p] of states) {
          if (vals.length >= 3) out.push([{ operation: op, conditions: vals }, p]);
          for (const [v, r] of item(p)) next.push([[...vals, v], r]);
        }
        states = next;
      }
    }
    return out;
  });

  operand: Rule = (pos) => this.rule('operand', pos, () => this.alt(
    this.seq(pos, ['niet', this.closed], (v) => ({ operation: 'NOT', value: v })),
    this.comparison(pos),
    this.arith(pos),
  ));

  comparison: Rule = (pos) => this.rule('comparison', pos, () => {
    const out: Result[] = [];
    const cmps = [['is meer dan', 'GREATER_THAN'], ['is minder dan', 'LESS_THAN'], ['is ten minste', 'GREATER_THAN_OR_EQUAL'], ['is ten hoogste', 'LESS_THAN_OR_EQUAL'], ['is', 'EQUALS']];
    for (const [s, p] of this.arith(pos)) {
      for (const [w, op] of cmps) {
        const q = this.word(p, w);
        if (q < 0) continue;
        for (const [v, r] of this.arith(q)) out.push([{ operation: op, subject: s, value: v }, r]);
      }
      let q = this.word(p, 'is niet');
      if (q >= 0) for (const [v, r] of this.arith(q)) out.push([{ operation: 'NOT', value: { operation: 'EQUALS', subject: s, value: v } }, r]);
      q = this.word(p, 'is bekend');
      if (q >= 0) out.push([{ operation: 'NOT', value: { operation: 'EQUALS', subject: s, value: null } }, q]);
      q = this.word(p, 'ontbreekt');
      if (q >= 0) out.push([{ operation: 'EQUALS', subject: s, value: null }, q]);
      q = this.word(p, 'is een van');
      if (q >= 0) for (const [vals, r] of this.parenList(q)) out.push([{ operation: 'IN', subject: s, values: vals }, r]);
      q = this.word(p, 'staat in');
      if (q >= 0) for (const [v, r] of this.closed(q)) out.push([{ operation: 'IN', subject: s, value: v }, r]);
      out.push(...this.subordinate(s, p));
    }
    return out;
  });

  // The same comparisons in a subordinate clause (after `als` and `waarvoor`),
  // with the verb at the end: `[a] waar is`, `[a] meer is dan [b]`. The parser
  // accepts them anywhere; the round trip still demands exactly one reading.
  subordinate(s: unknown, p: number): Result[] {
    const out: Result[] = [];
    const around: [string, string, string][] = [['ten minste', 'is', 'GREATER_THAN_OR_EQUAL'], ['ten hoogste', 'is', 'LESS_THAN_OR_EQUAL']];
    for (const [w, op] of [['meer is dan', 'GREATER_THAN'], ['minder is dan', 'LESS_THAN']]) {
      const q = this.word(p, w);
      if (q >= 0) for (const [v, r] of this.arith(q)) out.push([{ operation: op, subject: s, value: v }, r]);
    }
    for (const [w, end, op] of around) {
      const q = this.word(p, w);
      if (q < 0) continue;
      for (const [v, r] of this.arith(q)) {
        const e = this.word(r, end);
        if (e >= 0) out.push([{ operation: op, subject: s, value: v }, e]);
      }
    }
    for (const [v, r] of this.arith(p)) {
      const e = this.word(r, 'is');
      if (e >= 0) out.push([{ operation: 'EQUALS', subject: s, value: v }, e]);
    }
    let q = this.word(p, 'niet');
    if (q >= 0) {
      for (const [v, r] of this.arith(q)) {
        const e = this.word(r, 'is');
        if (e >= 0) out.push([{ operation: 'NOT', value: { operation: 'EQUALS', subject: s, value: v } }, e]);
      }
    }
    q = this.word(p, 'bekend is');
    if (q >= 0) out.push([{ operation: 'NOT', value: { operation: 'EQUALS', subject: s, value: null } }, q]);
    q = this.word(p, 'een van');
    if (q >= 0) {
      for (const [vals, r] of this.parenList(q)) {
        const e = this.word(r, 'is');
        if (e >= 0) out.push([{ operation: 'IN', subject: s, values: vals }, e]);
      }
    }
    q = this.word(p, 'in');
    if (q >= 0) {
      for (const [v, r] of this.closed(q)) {
        const e = this.word(r, 'staat');
        if (e >= 0) out.push([{ operation: 'IN', subject: s, value: v }, e]);
      }
    }
    return out;
  }

  arith: Rule = (pos) => this.rule('arith', pos, () => {
    const out: Result[] = [...this.closed(pos)];
    for (const [w, op] of [['plus', 'ADD'], ['min', 'SUBTRACT'], ['maal', 'MULTIPLY'], ['gedeeld door', 'DIVIDE']]) {
      for (const [vals, p] of this.sepBy(pos, this.closed, w, 2)) out.push([{ operation: op, values: vals }, p]);
    }
    out.push(...this.dateAdd(pos));
    out.push(...this.aggregate(pos));
    return out;
  });

  // FOREACH with ADD/MIN/MAX/ONLY/no combine: arithmetic level, because its
  // optional trailing `waarvoor` keeps it from being closed.
  aggregate: Rule = (pos) => {
    const C = this.closed;
    const local = this.localName;
    const filter: Rule = (p) => [[undefined, p], ...this.seq(p, ['waarvoor', C], (f) => f)];
    const out: Result[] = [];
    const forms: [string, string | undefined][] = [['de som van', 'ADD'], ['de kleinste waarde van', 'MIN'], ['de grootste waarde van', 'MAX'], ['de enige waarde van', 'ONLY'], ['de lijst van', undefined]];
    for (const [w, combine] of forms) {
      out.push(...this.seq(pos, [w, C, 'over elke', local, 'in', C, filter], (b, x, c, f) => {
        const n: Operation = { operation: 'FOREACH', collection: c, as: x, body: b };
        if (f !== undefined) n.filter = f;
        if (combine) n.combine = combine;
        return n;
      }));
    }
    return out;
  };

  dateAdd: Rule = (pos) => {
    const out: Result[] = [];
    const unitAt = (p: number): Result<string>[] => {
      const res: Result<string>[] = [];
      for (const [key, words] of Object.entries(PERIOD_WORDS)) {
        for (const w of words) {
          const q = this.word(p, w);
          if (q >= 0) res.push([key, q]);
        }
      }
      return res;
    };
    const offset = (p: number): Result<[string, unknown]>[] => {
      const res: Result<[string, unknown]>[] = [];
      for (const [v, q] of this.closed(p)) for (const [key, r] of unitAt(q)) res.push([[key, v], r]);
      return res;
    };
    // offsets: o | o en o | o , o , … en o
    const lists: Result<[string, unknown][]>[] = [];
    for (const [o, p] of offset(pos)) {
      lists.push([[o], p]);
      let states: Result<[string, unknown][]>[] = [[[o], p]];
      while (states.length) {
        const next: Result<[string, unknown][]>[] = [];
        for (const [os, q] of states) {
          const e = this.word(q, 'en');
          if (e >= 0) for (const [o2, r] of offset(e)) lists.push([[...os, o2], r]);
          const c = this.word(q, ',');
          if (c >= 0) for (const [o2, r] of offset(c)) next.push([[...os, o2], r]);
        }
        states = next;
      }
    }
    for (const [os, p] of lists) {
      const q = this.word(p, 'na');
      if (q < 0) continue;
      for (const [d, r] of this.closed(q)) {
        const node: Operation = { operation: 'DATE_ADD', date: d };
        for (const [key, v] of os) node[key] = v;
        out.push([node, r]);
      }
    }
    return out;
  };

  parenList: Rule<unknown[]> = (pos) => {
    const out: Result<unknown[]>[] = [];
    const open = this.word(pos, '(');
    if (open < 0) return out;
    const close = this.word(open, ')');
    if (close >= 0) out.push([[], close]);
    for (const [vals, p] of this.sepBy(open, this.expr, ';', 1)) {
      const q = this.word(p, ')');
      if (q >= 0) out.push([vals, q]);
    }
    return out;
  };

  closed: Rule = (pos) => this.rule('closed', pos, () => {
    const out: Result[] = [...this.atom(pos)];
    out.push(...this.seq(pos, ['(', this.expr, ')'], (v) => v));
    out.push(...this.func(pos));
    return out;
  });

  segment(pos: number): string | null {
    const tk = this.t[pos];
    if (!tk) return null;
    if (tk.t === 'name') return group(tk, 1).replace(/ /g, '_');
    if (tk.t === 'term') {
      const n = this.vocab.nameOf(group(tk, 1));
      if (n === null) throw new Error(`parse: unknown term ‹${group(tk, 1)}›`);
      return n;
    }
    return null;
  }

  name: Rule<string> = (pos) => this.rule('name', pos, () => {
    const first = this.segment(pos);
    if (first === null) return [];
    const out: Result<string>[] = [];
    let segs = [first];
    let p = pos + 1;
    out.push([`$${segs.slice().reverse().join('.')}`, p]);
    for (;;) {
      const q = this.word(p, 'van');
      if (q < 0) break;
      const s = this.segment(q);
      if (s === null) break;
      segs = [...segs, s];
      p = q + 1;
      out.push([`$${segs.slice().reverse().join('.')}`, p]);
    }
    return out;
  });

  // A name of one segment, without its `$`: a FOREACH variable.
  localName: Rule<string> = (p) => this.name(p).filter(([n]) => !n.includes('.')).map(([n, r]): Result<string> => [n.slice(1), r]);

  atom: Rule = (pos) => this.rule('atom', pos, () => {
    const out: Result[] = [...this.name(pos)];
    const tk = this.t[pos];
    if (!tk) return out;
    if (tk.t === 'str') out.push([group(tk, 1).replace(/\\(.)/g, '$1'), pos + 1]);
    if (tk.t === 'date') {
      const mo = MONTHS.indexOf(group(tk, 2)) + 1;
      out.push([`${group(tk, 3)}-${String(mo).padStart(2, '0')}-${group(tk, 1).padStart(2, '0')}`, pos + 1]);
    }
    if (tk.t === 'money') out.push([lit('money', group(tk, 1)), pos + 1]);
    if (tk.t === 'num') {
      if (group(tk, 2) === '%') out.push([lit('pct', group(tk, 1)), pos + 1]);
      else {
        out.push([lit('num', group(tk, 1)), pos + 1]);
        for (const [key, words] of Object.entries(PERIOD_WORDS)) {
          words.forEach((w, i) => {
            if (this.word(pos + 1, w) >= 0) out.push([lit('qty', group(tk, 1), { key, plural: i === 1 }), pos + 2]);
          });
        }
      }
    }
    if (tk.t === 'w') {
      const w = group(tk, 1);
      if (/^[A-Z]+$/.test(w)) out.push([{ __letter: w }, pos + 1]);
      if (w === 'waar') out.push([true, pos + 1]);
      if (w === 'onwaar') out.push([false, pos + 1]);
      if (w === 'geen' && this.word(pos, 'geen geval') < 0 && this.word(pos, 'geen termijn') < 0) out.push([null, pos + 1]);
    }
    return out;
  });

  func: Rule = (pos) => this.rule('func', pos, () => {
    const C = this.closed;
    const out: Result[] = [];
    // Short n-ary forms: `én ( … )`, `plus ( … )`.
    const shortForms: [string, string, string][] = [['én', 'AND', 'conditions'], ['óf', 'OR', 'conditions'], ['plus', 'ADD', 'values'], ['min', 'SUBTRACT', 'values'], ['maal', 'MULTIPLY', 'values'], ['gedeeld door', 'DIVIDE', 'values']];
    for (const [w, op, key] of shortForms) {
      const q = this.word(pos, w);
      if (q < 0) continue;
      for (const [vals, r] of this.parenList(q)) if (vals.length < 2) out.push([{ operation: op, [key]: vals }, r]);
    }
    for (const [w, op] of [['de kleinste van', 'MIN'], ['de grootste van', 'MAX']]) {
      const q = this.word(pos, w);
      if (q >= 0) for (const [vals, r] of this.parenList(q)) out.push([{ operation: op, values: vals }, r]);
    }
    out.push(...this.seq(pos, ['het deel van', C, 'boven', C], (a, b) => ({ operation: 'MAX', values: [0, { operation: 'SUBTRACT', values: [a, b] }] })));
    let q = this.word(pos, 'de lijst');
    if (q >= 0 && this.word(q, 'van') < 0) for (const [vals, r] of this.parenList(q)) out.push([{ operation: 'LIST', items: vals }, r]);
    // FOREACH
    const local = this.localName;
    const filter: Rule = (p) => [[undefined, p], ...this.seq(p, ['waarvoor', C], (f) => f)];
    const build = (combine: string, b: unknown, x: unknown, c: unknown, f: unknown): Operation => {
      const n: Operation = { operation: 'FOREACH', collection: c, as: x, body: b };
      if (f !== undefined) n.filter = f;
      if (combine) n.combine = combine;
      return n;
    };
    for (const [w, combine] of [['voor elke', 'AND'], ['voor ten minste één', 'OR']]) {
      out.push(...this.seq(pos, [w, local, 'in', C, filter, 'geldt', C], (x, c, f, b) => build(combine, b, x, c, f)));
    }
    out.push(...this.seq(pos, ['het aantal volle jaren tussen', C, 'en', C], (a, b) => ({ operation: 'AGE', date_of_birth: a, reference_date: b })));
    out.push(...this.seq(pos, ['geen termijn na', C], (d) => ({ operation: 'DATE_ADD', date: d })));
    // DATE: `1 januari van Y`
    const tk = this.t[pos];
    if (tk && tk.t === 'num' && group(tk, 2) === '' && /^\d+$/.test(group(tk, 1))) {
      MONTHS.forEach((m, i) => {
        const r = this.word(pos + 1, `${m} van`);
        if (r >= 0) for (const [y, s] of C(r)) out.push([{ operation: 'DATE', year: y, month: i + 1, day: Number(group(tk, 1)) }, s]);
      });
    }
    out.push(...this.seq(pos, ['de datum met jaar', C, ',', 'maand', C, 'en dag', C], (y, m, d) => ({ operation: 'DATE', year: y, month: m, day: d })));
    out.push(...this.seq(pos, ['de weekdag van', C], (d) => ({ operation: 'DAY_OF_WEEK', date: d })));
    for (const [w, unit] of [['dagen', 'days'], ['maanden', 'months'], ['jaren', 'years']]) {
      out.push(...this.seq(pos, [`het aantal ${w} van`, C, 'tot', C], (a, b) => ({ operation: 'DATE_DIFF', from: a, to: b, in: unit })));
    }
    out.push(...this.seq(pos, ['het verschil van', C, 'tot', C, 'in', this.name], (a, b, e) => ({ operation: 'DATE_DIFF', from: a, to: b, in: e })));
    for (const [w, part] of [['het jaartal van', 'year'], ['het maandnummer van', 'month'], ['het dagnummer van', 'day']]) {
      out.push(...this.seq(pos, [w, C], (d) => ({ operation: 'DATE_PART', date: d, in: part })));
    }
    for (const [w, part] of [['de eerste dag van het jaar waarin', 'year'], ['de eerste dag van de maand waarin', 'month']]) {
      out.push(...this.seq(pos, [w, C, 'valt'], (d) => ({ operation: 'START_OF', date: d, in: part })));
    }
    for (const [w, op] of [['naar boven afgerond', 'CEIL'], ['naar beneden afgerond', 'FLOOR'], ['afgerond', 'ROUND']]) {
      q = this.word(pos, `${w} op`);
      if (q < 0) continue;
      const precs: Result<number | PrecisionOf>[] = [];
      let r = this.word(q, 'hele eenheden');
      if (r >= 0) precs.push([0, r]);
      // Whole euros and cents: the precision follows from the unit of the value.
      r = this.word(q, "hele euro's");
      if (r >= 0) precs.push([(u) => (u === 'euro' ? 0 : u === 'eurocent' ? -2 : undefined), r]);
      r = this.word(q, 'hele centen');
      if (r >= 0) precs.push([(u) => (u === 'eurocent' ? 0 : undefined), r]);
      r = this.word(q, '1 decimaal');
      if (r >= 0) precs.push([1, r]);
      const t1 = this.t[q];
      if (t1 && t1.t === 'num' && /^\d+$/.test(group(t1, 1)) && group(t1, 2) === '' && this.word(q + 1, 'decimalen') >= 0) precs.push([Number(group(t1, 1)), q + 2]);
      r = this.word(q, 'veelvouden van');
      const t2 = this.t[r];
      if (r >= 0 && t2 && t2.t === 'num' && /^10+$/.test(group(t2, 1))) precs.push([-(group(t2, 1).length - 1), r + 1]);
      for (const [p, s] of precs) {
        for (const [v, e] of this.seq(s, ['(', this.expr, ')'], (x) => x)) {
          // A letter's unit is known only once the legend stands; precision too.
          const prec = typeof p === 'function' ? { __prec: p } : p;
          out.push([{ operation: op, value: v, precision: prec }, e]);
        }
      }
    }
    return out;
  });
}

function toNumber(text: string): number {
  return Number(text.replace(',', '.'));
}

function moneyValue(text: string, unit: Unit | null): number {
  const neg = text.startsWith('-');
  const [int = '', frac = ''] = text.replace('-', '').replace(/\./g, '').split(',');
  let digits = int + frac;
  let point = int.length;
  if (unit === 'eurocent') point += 2;
  while (digits.length < point) digits += '0';
  const s = `${digits.slice(0, point)}.${digits.slice(point)}`;
  return (neg ? -1 : 1) * Number(s);
}

/** Replaces the number placeholders now that every literal's unit is known. */
function isLit(node: unknown): node is LitPlaceholder {
  return isRecord(node) && typeof node.__lit === 'string';
}

function resolveLiterals(node: unknown, units: ReadonlyMap<string, Unit>, path: Path): unknown {
  if (Array.isArray(node)) return node.map((v: unknown, i) => resolveLiterals(v, units, [...path, i]));
  if (node === null || typeof node !== 'object') return node;
  if (isLit(node)) {
    const u = units.get(pathKey(path)) ?? null;
    const money = u === 'euro' || u === 'eurocent';
    const period = u && PERIOD_WORDS[u] ? u : null;
    const fail = (): never => { throw new Error(`parse: ${node.__lit} '${node.text}' does not fit unit ${u} at ${pathKey(path)}`); };
    switch (node.__lit) {
      case 'money': if (!money) fail(); return moneyValue(node.text, u);
      case 'pct': if (u !== 'percentage') fail(); return toNumber(node.text);
      case 'qty': {
        if (node.key !== period) fail();
        const v = toNumber(node.text);
        if (node.plural !== (v !== 1)) fail();
        return v;
      }
      case 'num': if (money || u === 'percentage' || period) fail(); return toNumber(node.text);
      default: return fail();
    }
  }
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(node)) {
    // The parser only builds `cases` as a list of `{ when, then }` objects.
    if (k === 'cases') out.cases = (v as Record<string, unknown>[]).map((c, i) => ({ ...(('when' in c) ? { when: resolveLiterals(c.when, units, [...path, 'cases', i, 'when']) } : {}), ...(('then' in c) ? { then: resolveLiterals(c.then, units, [...path, 'cases', i, 'then']) } : {}) }));
    else out[k] = resolveLiterals(v, units, [...path, k]);
  }
  return out;
}

/** Every way to read `A = … B = …` from `pos` to the end, as `[[letter, tree]…]`. */
type Definitions = [string, unknown][];

function legends(p: Parser, pos: number): Definitions[] {
  if (pos === p.t.length) return [[]];
  const tk = p.t[pos];
  if (!tk || tk.t !== 'w' || !/^[A-Z]+$/.test(group(tk, 1))) return [];
  const q = p.word(pos + 1, '=');
  if (q < 0) return [];
  const out: Definitions[] = [];
  for (const [v, r] of p.expr(q)) for (const rest of legends(p, r)) out.push([[group(tk, 1), v], ...rest]);
  return out;
}

/** The tree with every letter replaced by its definition; throws on a letter that is undefined, unused or defined twice, or on a cycle. */
function expandLegend(tree: unknown, defs: Definitions): unknown {
  const table = new Map<string, unknown>();
  for (const [l, v] of defs) {
    if (table.has(l)) throw new Error(`parse: letter ${l} defined twice`);
    table.set(l, v);
  }
  const used = new Set<string>();
  const expand = (node: unknown, stack: string[]): unknown => {
    if (Array.isArray(node)) return node.map((x: unknown) => expand(x, stack));
    if (node === null || typeof node !== 'object') return node;
    const letter = (node as { __letter?: unknown }).__letter;
    if (typeof letter === 'string' && letter) {
      const l = letter;
      if (!table.has(l)) throw new Error(`parse: letter ${l} not defined`);
      if (stack.includes(l)) throw new Error(`parse: letter ${l} refers to itself`);
      used.add(l);
      return expand(table.get(l), [...stack, l]);
    }
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(node)) out[k] = expand(v, stack);
    return out;
  };
  const out = expand(tree, []);
  for (const l of table.keys()) if (!used.has(l)) throw new Error(`parse: letter ${l} unused`);
  return out;
}

/** Fixes the precision of `hele euro's` and `hele centen` now that the letters are expanded. */
function resolvePrecision(node: unknown, declared: ReadonlyMap<string, Unit>): unknown {
  if (Array.isArray(node)) return node.map((x: unknown) => resolvePrecision(x, declared));
  if (node === null || typeof node !== 'object' || isLit(node)) return node;
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(node)) out[k] = resolvePrecision(v, declared);
  const prec = isRecord(out.precision) ? out.precision.__prec : undefined;
  if (typeof prec === 'function') {
    const p = (prec as PrecisionOf)(exprUnit(out.value, declared));
    if (p === undefined) throw new Error('parse: rounding does not fit the unit');
    out.precision = p;
  }
  return out;
}

/** All complete parses of an action reading: `[uitkomst] wordt: …`, optionally followed by `waarbij: A = …`. */
export function parseActionAll(text: string, { vocab, declared }: { vocab: Vocabulary; declared: ReadonlyMap<string, Unit> }): ParsedAction[] {
  const tokens = lex(text);
  const p = new Parser(tokens, vocab, declared);
  const outName = p.name(0).filter(([n]) => !n.includes('.'));
  const results: ParsedAction[] = [];
  for (const [n, q] of outName) {
    const w = p.word(q, 'wordt:');
    if (w < 0) continue;
    for (const [v0, r] of p.expr(w)) {
      let defsList: Definitions[];
      if (r === tokens.length) defsList = [[]];
      else {
        const wb = p.word(r, 'waarbij:');
        if (wb < 0) continue;
        defsList = legends(p, wb).filter((d) => d.length > 0);
      }
      const output = n.slice(1);
      for (const defs of defsList) try {
        const v = resolvePrecision(expandLegend(v0, defs), declared);
        const units = literalUnits(v, declared.get(output) ?? null, declared, ['value']);
        results.push({ output, value: resolveLiterals(v, units, ['value']) });
      } catch (e) {
        if (!(e instanceof Error && e.message.startsWith('parse:'))) throw e;
      }
    }
  }
  // The same tree reached along two grammar paths is one parse.
  const seen = new Map<string, ParsedAction>();
  for (const r of results) seen.set(JSON.stringify(r), r);
  return [...seen.values()];
}

export function parseAction(text: string, ctx: Parameters<typeof parseActionAll>[1]): ParsedAction {
  const all = parseActionAll(text, ctx);
  if (all.length !== 1) throw new Error(`parse: ${all.length} readings of:\n${text}`);
  // Exactly one parse, checked above.
  return all[0]!;
}

export { isReference };
