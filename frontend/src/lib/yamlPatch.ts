/**
 * Comment-preserving patch of one article inside a law YAML document.
 *
 * The editor used to save by `yaml.dump`-ing the whole re-assembled document.
 * That drops every `#` comment, refolds long lines and can reorder keys, so a
 * one-word edit became a whole-file diff. For corpora that are also worked on
 * outside the editor (by hand or by an agent, in a git clone) that guarantees
 * merge conflicts on the next sync.
 *
 * A full parse → stringify round trip with a comment-aware library is not
 * enough either: it keeps comments, but it does not reproduce the original
 * bytes (line folding, quoting and indentation all drift). So this module
 * splices instead: it locates the source range of what changed and replaces
 * only those bytes with a freshly rendered `key: value`. Inside
 * `machine_readable` it descends to the smallest changed subtree. Every byte
 * outside the changed ranges is the input, untouched.
 *
 * Returns `null` when the document cannot be patched safely (parse error,
 * article not found, an unexpected node shape). Callers fall back to their
 * previous behaviour in that case.
 */
import { Document, isMap, isNode, isScalar, isSeq, parseDocument, Scalar } from 'yaml';
import type { Pair, YAMLMap, YAMLSeq } from 'yaml';

type Range = [number, number, number];
type AnyMap = YAMLMap<unknown, unknown>;
type AnySeq = YAMLSeq<unknown>;
type AnyPair = Pair<unknown, unknown>;
type ParsedDoc = Document.Parsed;

interface Style {
  indent: number;
  indentSeq: boolean;
}

interface Edit {
  start: number;
  end: number;
  insert: string;
}

/** What `patchArticle` changes. Only the keys present are touched. */
export interface ArticleChanges {
  text?: string;
  /** `undefined` means "not changed"; use `deleteMachineReadable` to remove it. */
  machine_readable?: unknown;
  deleteMachineReadable?: boolean;
}

function rangeOf(node: unknown): Range | undefined {
  return isNode(node) ? (node.range as Range | null | undefined) ?? undefined : undefined;
}

/** Column (0-based) of `offset` in `src`. */
function columnOf(src: string, offset: number): number {
  return offset - (src.lastIndexOf('\n', offset - 1) + 1);
}

/** Offset of the start of the line that contains `offset`. */
function lineStart(src: string, offset: number): number {
  return src.lastIndexOf('\n', offset - 1) + 1;
}

/**
 * End of a node's content, with trailing whitespace and newlines backed off.
 * Block scalars report a range that runs through their final newline; the
 * splice must leave that newline (and whatever follows) where it is.
 */
function contentEnd(src: string, end: number): number {
  let e = end;
  while (e > 0 && (src[e - 1] === '\n' || src[e - 1] === ' ' || src[e - 1] === '\r')) e--;
  return e;
}

function keyName(pair: AnyPair): string | null {
  return isScalar(pair.key) ? String(pair.key.value) : null;
}

/** Find the `articles[*]` map whose `number` equals `articleNumber`. */
function findArticle(doc: ParsedDoc, articleNumber: string | number) {
  const articles = doc.get('articles', true);
  if (!isSeq(articles)) return null;
  const wanted = String(articleNumber);
  for (const item of articles.items) {
    if (!isMap(item)) continue;
    const num = item.get('number');
    if (num != null && String(num) === wanted) {
      return { articles: articles as AnySeq, article: item as AnyMap };
    }
  }
  return null;
}

/**
 * Infer the indentation step and sequence style the file is written in, so a
 * rendered fragment reads like its neighbours. Falls back to the conventional
 * 2-space, indented-sequence style.
 */
function detectStyle(src: string, doc: ParsedDoc, article: AnyMap): Style {
  let indentSeq = true;
  const root = doc.contents;
  const articlesPair = isMap(root)
    ? (root.items as AnyPair[]).find((p) => keyName(p) === 'articles')
    : undefined;
  const articlesKey = rangeOf(articlesPair?.key);
  const articleRange = rangeOf(article);
  if (articlesKey && articleRange) {
    const keyCol = columnOf(src, articlesKey[0]);
    // The map's range starts after "- ", so the dash sits two columns left.
    const dashCol = columnOf(src, articleRange[0]) - 2;
    indentSeq = dashCol > keyCol;
  }
  let indent = 2;
  for (const pair of article.items) {
    const v = pair.value;
    const keyRange = rangeOf(pair.key);
    if (!keyRange || !(isMap(v) || isSeq(v)) || !v.items.length) continue;
    const child = v.items[0];
    const childStart = isMap(v) ? rangeOf((child as AnyPair).key)?.[0] : rangeOf(child)?.[0];
    if (childStart == null) continue;
    let step = columnOf(src, childStart) - columnOf(src, keyRange[0]);
    // A sequence child's range starts after "- ".
    if (isSeq(v)) step -= indentSeq ? 2 : 0;
    if (step > 0) {
      indent = step;
      break;
    }
  }
  return { indent, indentSeq };
}

/** Normalise a value for equality: js-yaml turns unquoted dates into Date. */
function norm(v: unknown): unknown {
  if (v instanceof Date) {
    const iso = v.toISOString();
    return iso.endsWith('T00:00:00.000Z') ? iso.slice(0, 10) : iso;
  }
  if (Array.isArray(v)) return v.map(norm);
  if (v && typeof v === 'object') {
    const o: Record<string, unknown> = {};
    for (const k of Object.keys(v)) o[k] = norm((v as Record<string, unknown>)[k]);
    return o;
  }
  return v;
}

function same(a: unknown, b: unknown): boolean {
  return JSON.stringify(norm(a)) === JSON.stringify(norm(b));
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return v != null && typeof v === 'object' && !Array.isArray(v) && !(v instanceof Date);
}

/**
 * Render `key: value` as a block fragment. The first line has no leading
 * indentation (it lands at the key's original column); every following
 * non-empty line is shifted right by `column`.
 */
function renderPair(key: string, value: unknown, column: number, style: Style, previous: unknown): string {
  const doc = new Document({ [key]: norm(value) });
  // Keep a block-literal text block literal, so the diff stays about words.
  const pair = (doc.contents as AnyMap | null)?.items[0];
  if (typeof value === 'string' && pair && isScalar(pair.value)) {
    if (isScalar(previous) && previous.type) {
      pair.value.type = previous.type;
    } else if (value.includes('\n')) {
      pair.value.type = Scalar.BLOCK_LITERAL;
    }
  }
  const text = doc
    .toString({ indent: style.indent, indentSeq: style.indentSeq, lineWidth: 0, minContentWidth: 0 })
    .replace(/\n+$/, '');
  const pad = ' '.repeat(column);
  return text
    .split('\n')
    .map((line, i) => (i === 0 || line === '' ? line : pad + line))
    .join('\n');
}

/** Thrown when a node cannot be patched in place; the parent re-renders it. */
const RERENDER = Symbol('rerender');

/**
 * Collects byte-range edits that turn the source under a node into a value,
 * re-rendering as little as possible so comments and formatting of unchanged
 * siblings survive. Throws `RERENDER` when a node has to be rendered whole.
 */
class Patcher {
  readonly edits: Edit[] = [];

  constructor(
    private readonly src: string,
    private readonly style: Style,
    private readonly doc: ParsedDoc,
  ) {}

  js(node: unknown): unknown {
    return isNode(node) ? node.toJS(this.doc) : (node ?? null);
  }

  private pairBounds(pair: AnyPair): { keyStart: number; end: number } {
    const keyStart = rangeOf(pair.key)?.[0];
    const valueEnd = rangeOf(pair.value)?.[1] ?? rangeOf(pair.key)?.[2];
    if (keyStart == null || valueEnd == null) throw RERENDER;
    return { keyStart, end: contentEnd(this.src, valueEnd) };
  }

  replacePair(pair: AnyPair, key: string, value: unknown): void {
    const { keyStart, end } = this.pairBounds(pair);
    const rendered = renderPair(key, value, columnOf(this.src, keyStart), this.style, pair.value);
    this.edits.push({ start: keyStart, end, insert: rendered });
  }

  deletePair(pair: AnyPair): void {
    const { keyStart, end } = this.pairBounds(pair);
    const from = lineStart(this.src, keyStart);
    // Only a pair that starts its own line can be cut as whole lines.
    if (this.src.slice(from, keyStart).trim() !== '') throw RERENDER;
    const nl = this.src.indexOf('\n', end);
    this.edits.push({ start: from, end: nl < 0 ? this.src.length : nl + 1, insert: '' });
  }

  appendPair(map: AnyMap, key: string, value: unknown): void {
    const last = map.items[map.items.length - 1];
    const firstKey = rangeOf(map.items[0]?.key);
    const lastEnd = rangeOf(last?.value)?.[1] ?? rangeOf(last?.key)?.[2];
    if (lastEnd == null || !firstKey) throw RERENDER;
    const column = columnOf(this.src, firstKey[0]);
    // End of the line the last value ends on, so a trailing `# comment` on
    // that line stays with the value it describes instead of moving onto the
    // appended key.
    const nl = this.src.indexOf('\n', contentEnd(this.src, lastEnd));
    const at = nl < 0 ? this.src.length : nl;
    const rendered = renderPair(key, value, column, this.style, null);
    this.edits.push({ start: at, end: at, insert: `\n${' '.repeat(column)}${rendered}` });
  }

  /** Reconcile a block map with `obj`, key by key. */
  map(node: unknown, obj: unknown): void {
    if (!isMap(node) || node.flow || !isPlainObject(obj)) throw RERENDER;
    const map = node as AnyMap;
    const seen = new Set<string>();
    let removed = 0;
    for (const pair of map.items) {
      const key = keyName(pair);
      if (key == null) throw RERENDER;
      seen.add(key);
      if (!Object.prototype.hasOwnProperty.call(obj, key)) {
        removed += 1;
        this.deletePair(pair);
        continue;
      }
      if (same(this.js(pair.value), obj[key])) continue;
      this.child(pair, key, obj[key]);
    }
    // Every existing key is being removed. Cutting them all would leave a
    // bare `key:` that parses as null instead of `{}`, and with keys to add
    // there is no anchor left to append after. Let the parent re-render.
    if (map.items.length - removed === 0) throw RERENDER;
    for (const key of Object.keys(obj)) {
      if (!seen.has(key)) this.appendPair(map, key, obj[key]);
    }
  }

  /** Reconcile one pair's value, re-rendering the pair if it can't be patched. */
  child(pair: AnyPair, key: string, value: unknown): void {
    const mark = this.edits.length;
    try {
      this.value(pair.value, value);
    } catch (e) {
      if (e !== RERENDER) throw e;
      this.edits.length = mark;
      this.replacePair(pair, key, value);
    }
  }

  value(node: unknown, value: unknown): void {
    if (isMap(node)) return this.map(node, value);
    if (isSeq(node)) return this.seq(node as AnySeq, value);
    throw RERENDER;
  }

  /** Reconcile a block sequence of equal length item by item. */
  seq(node: AnySeq, arr: unknown): void {
    if (node.flow || !Array.isArray(arr) || arr.length !== node.items.length) throw RERENDER;
    node.items.forEach((item, i) => {
      const next: unknown = arr[i];
      if (same(this.js(item), next)) return;
      if (isMap(item) && isPlainObject(next)) {
        this.map(item, next);
        return;
      }
      const range = rangeOf(item);
      if (isScalar(item) && range && (next == null || typeof next !== 'object')) {
        const text = new Document(norm(next)).toString({ lineWidth: 0 }).replace(/\n+$/, '');
        if (text.includes('\n')) throw RERENDER;
        this.edits.push({ start: range[0], end: contentEnd(this.src, range[1]), insert: text });
        return;
      }
      throw RERENDER;
    });
  }
}

// The editor recomputes the patched document on every keystroke against the
// same stored text; parsing that text once instead of per keystroke halves the
// cost on the largest laws. The patcher only reads the parsed document.
let cache: { src: string | null; doc: ParsedDoc | null } = { src: null, doc: null };
function parseCached(src: string): ParsedDoc | null {
  if (cache.src !== src) {
    let doc: ParsedDoc | null = null;
    try {
      doc = parseDocument(src);
      if (doc.errors.length) doc = null;
    } catch {
      doc = null;
    }
    cache = { src, doc };
  }
  return cache.doc;
}

/**
 * Patch fields of a single article, preserving every byte of the document
 * outside the changed parts. Inside `machine_readable` the patch descends to
 * the smallest changed subtree, so comments next to unchanged definitions or
 * actions survive an edit elsewhere in the same article.
 */
export function patchArticle(
  rawYaml: string,
  articleNumber: string | number,
  changes: ArticleChanges,
): string | null {
  if (typeof rawYaml !== 'string') return null;
  const doc = parseCached(rawYaml);
  if (!doc) return null;
  const found = findArticle(doc, articleNumber);
  if (!found) return null;
  const { article } = found;
  if (!rangeOf(article.items[0]?.key)) return null;
  const patcher = new Patcher(rawYaml, detectStyle(rawYaml, doc, article), doc);

  const wanted: [string, unknown][] = [];
  if (Object.prototype.hasOwnProperty.call(changes, 'text')) wanted.push(['text', changes.text]);
  if (!changes.deleteMachineReadable && changes.machine_readable !== undefined) {
    wanted.push(['machine_readable', changes.machine_readable]);
  }

  try {
    for (const [key, value] of wanted) {
      const pair = article.items.find((p) => keyName(p) === key);
      if (!pair) {
        patcher.appendPair(article, key, value);
        continue;
      }
      if (same(patcher.js(pair.value), value)) continue;
      if (key === 'text') patcher.replacePair(pair, key, value);
      else patcher.child(pair, key, value);
    }
    if (changes.deleteMachineReadable) {
      const pair = article.items.find((p) => keyName(p) === 'machine_readable');
      if (pair) patcher.deletePair(pair);
    }
  } catch (e) {
    if (e === RERENDER) return null;
    throw e;
  }

  const { edits } = patcher;
  if (!edits.length) return rawYaml;
  // Apply right-to-left so earlier offsets stay valid. Ranges never overlap:
  // every edit belongs to a distinct pair or item.
  edits.sort((a, b) => b.start - a.start);
  let out = rawYaml;
  for (const e of edits) out = out.slice(0, e.start) + e.insert + out.slice(e.end);

  // Self-check: the result must still parse. Anything else is a bug here, and
  // the caller's fallback is better than writing a broken file.
  if (parseDocument(out).errors.length) return null;
  return out;
}
