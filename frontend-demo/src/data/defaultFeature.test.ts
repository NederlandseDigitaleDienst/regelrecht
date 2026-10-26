import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import * as yaml from 'js-yaml';
import type { DemoConfig, DemoProfileConfig } from './loadCorpus';

// A `default_feature` that names no file falls back to whichever feature sorts
// first, silently: a new feature file for the same law then takes over the
// presenter's opening scenario.
const root = path.resolve(import.meta.dirname, '../../..');
const config = yaml.load(fs.readFileSync(path.join(root, 'corpus/demo/demo-config.yaml'), 'utf8')) as DemoConfig;

describe('default_feature of each profile', () => {
  // `profiles` is required in the real config; `?? {}` would let the test pass on nothing.
  const profiles = Object.entries(config.profiles!).filter((e): e is [string, DemoProfileConfig & { default_feature: string }] => !!e[1].default_feature);

  it.each(profiles)('%s names an existing feature file', (_id, p) => {
    expect(fs.existsSync(path.join(root, 'corpus/demo/regulation/nl', p.default_feature))).toBe(true);
  });
});
