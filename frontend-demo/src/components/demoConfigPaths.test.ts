import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import * as yaml from 'js-yaml';
import { matches, nodePaths } from '@regelrecht/frontend-shared/yamlExpand';

// Een pad dat na een wijziging in de wet nergens meer op wijst, klapt stil
// niets open: de presentator merkt het pas tijdens de demo.
describe('elk geconfigureerd pad wijst naar een knoop in de wet', () => {
  const root = path.resolve(import.meta.dirname, '../../..');
  const config = yaml.load(fs.readFileSync(path.join(root, 'corpus/demo/demo-config.yaml'), 'utf8')) as Record<
    string,
    Record<string, string[]> | undefined
  >;

  /** Every version of every demo law, by `$id`. */
  const versions = new Map<string, unknown[]>();
  for (const file of fs.readdirSync(path.join(root, 'corpus/demo/regulation'), { recursive: true, encoding: 'utf8' })) {
    if (!file.endsWith('.yaml')) continue;
    const law = yaml.load(fs.readFileSync(path.join(root, 'corpus/demo/regulation', file), 'utf8')) as { $id?: string } | null;
    if (!law?.$id) continue;
    versions.set(law.$id, [...(versions.get(law.$id) ?? []), law]);
  }

  const cases = ['expanded_paths', 'folded_paths'].flatMap((kind) =>
    Object.entries(config[kind] ?? {}).flatMap(([law, patterns]) => patterns.map((pattern): [string, string, string] => [kind, law, pattern])));

  it.each(cases)('%s van %s: %s', (_kind, law, pattern) => {
    const laws = versions.get(law) ?? [];
    expect(laws.length).toBeGreaterThan(0);
    const found = laws.some((l) => nodePaths(l).some((p) => matches(pattern, p)));
    expect(found).toBe(true);
  });
});
