/**
 * A minimal line diff (LCS) for the law-change sheet. No dependency; enough
 * to show the added and removed lines between two YAML texts.
 */

/** One line of a diff. `gap` stands for context that `compactDiff` left out. */
export interface DiffLine {
  type: 'context' | 'add' | 'del' | 'gap';
  text: string;
}

/** A changed article text, as `articleTextDiff` reports it. */
export interface ArticleTextChange {
  article: string | number;
  before: string;
  after: string;
}

/** A changed definition value, as `definitionDiff` reports it. */
export interface DefinitionChange {
  article: string | number;
  name: string;
  before: unknown;
  after: unknown;
}

/** The part of a parsed rulework the two document diffs read. */
interface DiffDocument {
  articles?: {
    number: string | number;
    text?: unknown;
    machine_readable?: { definitions?: Record<string, { value?: unknown } | null | undefined> };
  }[];
}

export function lineDiff(a: string, b: string): DiffLine[] {
  const A = a.split('\n');
  const B = b.split('\n');
  const n = A.length;
  const m = B.length;

  // LCS lengths.
  const dp = Array.from({ length: n + 1 }, () => new Int32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] = A[i] === B[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (A[i] === B[j]) {
      out.push({ type: 'context', text: A[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      out.push({ type: 'del', text: A[i] });
      i++;
    } else {
      out.push({ type: 'add', text: B[j] });
      j++;
    }
  }
  while (i < n) out.push({ type: 'del', text: A[i++] });
  while (j < m) out.push({ type: 'add', text: B[j++] });
  return out;
}

/** Only the changed blocks (context trimmed to 2 lines around each change). */
export function compactDiff(a: string, b: string, contextLines = 2): DiffLine[] {
  const full = lineDiff(a, b);
  const keep: boolean[] = new Array(full.length).fill(false);
  full.forEach((d, idx) => {
    if (d.type !== 'context') {
      for (let k = Math.max(0, idx - contextLines); k <= Math.min(full.length - 1, idx + contextLines); k++) {
        keep[k] = true;
      }
    }
  });
  const out: DiffLine[] = [];
  let gap = false;
  full.forEach((d, idx) => {
    if (keep[idx]) {
      out.push(d);
      gap = false;
    } else if (!gap) {
      out.push({ type: 'gap', text: '…' });
      gap = true;
    }
  });
  return out;
}

/**
 * Articles whose law text changed, as [{ article, before, after }].
 *
 * Why apart from definitionDiff: a law is its text, and a change that is only
 * in the formula tells a different law than the prose around it. Whoever reads
 * the diff has to see both.
 */
export function articleTextDiff(
  baseDoc: DiffDocument | null | undefined,
  currentDoc: DiffDocument | null | undefined,
): ArticleTextChange[] {
  const changes: ArticleTextChange[] = [];
  const curByNumber = new Map((currentDoc?.articles ?? []).map((a) => [String(a.number), a]));
  for (const art of baseDoc?.articles ?? []) {
    const cur = curByNumber.get(String(art.number));
    if (!cur) continue;
    const before = typeof art.text === 'string' ? art.text : null;
    const after = typeof cur.text === 'string' ? cur.text : null;
    if (before !== null && after !== null && before !== after) {
      changes.push({ article: art.number, before, after });
    }
  }
  return changes;
}

/**
 * Differences in definition values between two parsed documents, as readable
 * "article X: name: before → after" lines.
 */
export function definitionDiff(
  baseDoc: DiffDocument | null | undefined,
  currentDoc: DiffDocument | null | undefined,
): DefinitionChange[] {
  const changes: DefinitionChange[] = [];
  const baseArticles = baseDoc?.articles ?? [];
  const curArticles = currentDoc?.articles ?? [];
  const curByNumber = new Map(curArticles.map((a) => [String(a.number), a]));

  for (const art of baseArticles) {
    const baseDefs = art?.machine_readable?.definitions;
    if (!baseDefs) continue;
    const curArt = curByNumber.get(String(art.number));
    const curDefs = curArt?.machine_readable?.definitions ?? {};
    for (const [name, def] of Object.entries(baseDefs)) {
      const before = def?.value;
      const after = curDefs[name]?.value;
      if (after !== undefined && JSON.stringify(before) !== JSON.stringify(after)) {
        changes.push({ article: art.number, name, before, after });
      }
    }
  }
  return changes;
}
