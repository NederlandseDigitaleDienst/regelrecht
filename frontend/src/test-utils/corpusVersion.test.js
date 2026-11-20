import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { extractDateFromPath, pickBestVersion } from './corpusVersion.js';
import { loadCorpus, loadCorpusVersions } from './corpusOnDisk.js';

// A law whose filename date and publication_date deliberately disagree: the
// server keys on the filename, so the two must not be interchangeable here.
function lawYaml(id, publicationDate) {
  return `$id: ${id}\nregulatory_layer: WET\npublication_date: '${publicationDate}'\narticles: []\n`;
}

let root;

beforeAll(() => {
  root = mkdtempSync(join(tmpdir(), 'corpus-version-'));
  const dir = join(root, 'wet', 'test_wet');
  mkdirSync(dir, { recursive: true });
  // In force since 2020, superseded on paper by a version that only takes
  // effect in 2099 — and whose publication_date is the highest of the two.
  writeFileSync(join(dir, '2020-01-01.yaml'), lawYaml('test_wet', '2019-12-01'));
  writeFileSync(join(dir, '2099-01-01.yaml'), lawYaml('test_wet', '2098-12-01'));

  const undatedDir = join(root, 'wet', 'ander_wet');
  mkdirSync(undatedDir, { recursive: true });
  writeFileSync(join(undatedDir, 'concept.yaml'), lawYaml('ander_wet', '2030-01-01'));
  writeFileSync(join(undatedDir, '2021-06-01.yaml'), lawYaml('ander_wet', '2001-01-01'));
});

afterAll(() => {
  rmSync(root, { recursive: true, force: true });
});

describe('loadCorpus', () => {
  it('serves the version in force today, not the latest publication_date', () => {
    const entry = loadCorpus(root, '2025-06-01').get('test_wet');
    expect(entry.path).toContain('2020-01-01.yaml');
    expect(entry.content).toContain("publication_date: '2019-12-01'");
  });

  it('follows today past a future version once it takes effect', () => {
    expect(loadCorpus(root, '2099-06-01').get('test_wet').path).toContain('2099-01-01.yaml');
  });

  it('prefers a dated file over an undated one', () => {
    expect(loadCorpus(root, '2025-06-01').get('ander_wet').path).toContain('2021-06-01.yaml');
  });
});

describe('loadCorpusVersions', () => {
  it('orders by filename date, newest first and undated last', () => {
    const versions = loadCorpusVersions(root);
    expect(versions.get('test_wet')).toEqual([
      lawYaml('test_wet', '2098-12-01'),
      lawYaml('test_wet', '2019-12-01'),
    ]);
    // The undated concept has the highest publication_date of its pair and
    // still sorts last.
    expect(versions.get('ander_wet')).toEqual([
      lawYaml('ander_wet', '2001-01-01'),
      lawYaml('ander_wet', '2030-01-01'),
    ]);
  });
});

describe('extractDateFromPath', () => {
  it('reads the date off the filename', () => {
    expect(extractDateFromPath('/corpus/wet/my_law/2025-01-01.yaml')).toBe('2025-01-01');
  });

  it('rejects anything that is not a dated yaml filename', () => {
    expect(extractDateFromPath('/corpus/wet/my_law/concept.yaml')).toBeNull();
    expect(extractDateFromPath('/corpus/2025-01-01/my_law.yaml')).toBeNull();
    expect(extractDateFromPath('/corpus/wet/my_law/2025-1-1.yaml')).toBeNull();
    expect(extractDateFromPath('/corpus/wet/my_law/2025-01-01.yml')).toBeNull();
  });
});

describe('pickBestVersion', () => {
  const today = '2025-06-01';

  it('lets an in-force version beat a future one, in either direction', () => {
    expect(pickBestVersion('2099-01-01', '2020-01-01', today)).toBe(true);
    expect(pickBestVersion('2020-01-01', '2099-01-01', today)).toBe(false);
  });

  it('takes the latest date within one group', () => {
    expect(pickBestVersion('2020-01-01', '2024-01-01', today)).toBe(true);
    expect(pickBestVersion('2024-01-01', '2020-01-01', today)).toBe(false);
    expect(pickBestVersion('2099-01-01', '2100-01-01', today)).toBe(true);
  });

  it('lets any dated version beat an undated one', () => {
    expect(pickBestVersion(null, '2099-01-01', today)).toBe(true);
    expect(pickBestVersion('2020-01-01', null, today)).toBe(false);
    expect(pickBestVersion(null, null, today)).toBe(false);
  });

  it('treats a version starting today as in force', () => {
    expect(pickBestVersion('2099-01-01', today, today)).toBe(true);
  });
});
