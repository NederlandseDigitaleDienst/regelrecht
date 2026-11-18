/**
 * What a profile in demo-config.yaml points at has to exist in the corpus: a
 * name that matches nothing falls back without a sound (a switch to Claudia
 * once opened the Alcoholwet). `default_feature` has its own test
 * (`defaultFeature.test.ts`); this one covers the law and the graph.
 *
 * Reads the corpus, not `public/data/`, for the reason `profilesOverlay.test.ts`
 * gives: CI runs the tests without the build that copies it there.
 */
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as yaml from 'js-yaml';
import { describe, expect, it } from 'vitest';
import type { DemoConfig, DemoProfileConfig } from './loadCorpus';

const here = dirname(fileURLToPath(import.meta.url));
const demoDir = resolve(here, '..', '..', '..', 'corpus', 'demo');
const lawsDir = join(demoDir, 'regulation', 'nl');
// The real config has `profiles`; the destructuring would throw without them, as before.
const { profiles } = yaml.load(readFileSync(join(demoDir, 'demo-config.yaml'), 'utf8')) as DemoConfig & {
  profiles: Record<string, DemoProfileConfig>;
};

/** Every `$id` in the demo corpus. */
function lawIds(dir: string = lawsDir): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = join(dir, e.name);
    if (e.isDirectory()) return lawIds(p);
    if (!e.name.endsWith('.yaml')) return [];
    const id = readFileSync(p, 'utf8').match(/^\$id: '?([^'\n]+)'?$/m)?.[1];
    return id ? [id] : [];
  });
}
const ids = new Set(lawIds());

describe.each(Object.entries(profiles))('profile %s', (key, profile) => {
  it('opens a default law that exists for its service', () => {
    const defaultLaw = profile.default_law;
    if (!defaultLaw) throw new Error(`profile ${key} has no default_law`);
    const { law_path: lawPath, service } = defaultLaw;
    const dir = join(lawsDir, lawPath);
    expect(existsSync(dir) && readdirSync(dir).some((f) => f.startsWith(`${service}-`)), `${service}/${lawPath}`).toBe(true);
  });

  it('draws and focuses laws that exist', () => {
    for (const id of [...(profile.graph_laws ?? []), ...(profile.graph_focus ? [profile.graph_focus] : [])]) {
      expect(ids.has(id), id).toBe(true);
    }
  });
});
