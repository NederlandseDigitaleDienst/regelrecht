// Reads every action out of a directory of regelwerk YAML files, for the
// round-trip test and the per-corpus report. Node only; test support.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { load } from 'js-yaml';
import { isRecord } from '../types';

/** One action of a corpus, with where it stands. */
export interface CorpusAction {
  file: string;
  article: Record<string, unknown>;
  action: unknown;
  index: number;
}

function* yamlFiles(dir: string): Generator<string> {
  for (const entry of readdirSync(dir).sort()) {
    const p = join(dir, entry);
    const st = statSync(p);
    if (st.isDirectory()) yield* yamlFiles(p);
    else if (/\.ya?ml$/.test(entry)) yield p;
  }
}

/** `[{ file, article, action, index }]` for every action in every regelwerk under `dir`. */
export function corpusActions(dir: string): CorpusAction[] {
  const out: CorpusAction[] = [];
  for (const file of yamlFiles(dir)) {
    let doc: unknown;
    try {
      // js-yaml, like the editor (useLaw.ts): an unquoted date becomes a Date.
      doc = load(readFileSync(file, 'utf8'));
    } catch {
      continue;
    }
    if (!isRecord(doc) || !Array.isArray(doc.articles)) continue;
    for (const article of doc.articles as unknown[]) {
      if (!isRecord(article)) continue;
      const mr = article.machine_readable;
      const ex = isRecord(mr) ? mr.execution : undefined;
      const actions = isRecord(ex) ? ex.actions : undefined;
      if (!Array.isArray(actions)) continue;
      actions.forEach((action: unknown, index) => out.push({ file: relative(dir, file), article, action, index }));
    }
  }
  return out;
}
