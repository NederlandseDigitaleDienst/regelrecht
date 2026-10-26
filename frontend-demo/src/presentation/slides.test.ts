/**
 * The slides in demo-config.yaml make claims about the demo next to them, and
 * nothing else checks that those claims still hold.
 *
 * Two went stale without a sound. The slide on applying pulsed
 * `nldd-button[text="Gegevens aanvullen"]` after the button had been renamed
 * "Aanvullen", so the highlight never appeared (and never had in English or
 * Frisian). The simulation slide asked what a law does to fifty thousand
 * people, while a run stops at two thousand.
 *
 * Reads the corpus, not `public/data/`, for the reason `profilesOverlay.test.ts`
 * gives: CI runs the tests without the build that copies it there.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as yaml from 'js-yaml';
import { describe, expect, it } from 'vitest';
import { MAX_POPULATION } from '../simulation/population';
import type { DemoConfig } from '../data/loadCorpus';

const here = dirname(fileURLToPath(import.meta.url));
const src = resolve(here, '..');
const demoDir = resolve(here, '..', '..', '..', 'corpus', 'demo');

// The parsed YAML is trusted to have the shape loadCorpus gives it; a slide
// list that is missing fails the first test below.
const slides = (yaml.load(readFileSync(join(demoDir, 'demo-config.yaml'), 'utf8')) as DemoConfig).slides ?? [];
/** A translation overlay: flat `slides.<i>.title`-style keys to text. */
type Overlay = Record<string, string | undefined>;
const overlays: Record<string, Overlay> = {
  en: (yaml.load(readFileSync(join(demoDir, 'i18n', 'en.yaml'), 'utf8')) as Overlay | undefined) ?? {},
  fy: (yaml.load(readFileSync(join(demoDir, 'i18n', 'fy.yaml'), 'utf8')) as Overlay | undefined) ?? {},
};

/** Every .vue file under src, concatenated: where a `data-highlight` anchor has to live. */
function vueSources(dir: string = src): string {
  return readdirSync(dir, { withFileTypes: true })
    .flatMap((e) => (e.isDirectory() ? [vueSources(join(dir, e.name))] : e.name.endsWith('.vue') ? [readFileSync(join(dir, e.name), 'utf8')] : []))
    .join('\n');
}

describe('slide highlights', () => {
  const highlighted = slides.map((s, i): [number, string | undefined] => [i, s.highlight]).filter(([, h]) => h) as [number, string][];

  it('has slides to check', () => {
    expect(highlighted.length).toBeGreaterThan(0);
  });

  it.each(highlighted)('slides[%i] does not select on a label', (_, selector) => {
    // A label is translated and gets reworded; either one turns the highlight
    // off without an error, because `querySelectorAll` just finds nothing.
    // Point at the element with `data-highlight` instead.
    expect(selector).not.toMatch(/\[(text|label|aria-label|title)\s*[~|^$*]?=/);
  });

  it.each(highlighted)('slides[%i] points at an anchor that exists', (_, selector) => {
    const anchors = [...selector.matchAll(/\[data-highlight=["']?([\w-]+)["']?\]/g)].map((m) => m[1]);
    const vue = vueSources();
    for (const anchor of anchors) {
      expect(vue, `no element carries data-highlight="${anchor}"`).toContain(`data-highlight="${anchor}"`);
    }
  });
});

describe('the simulation slide', () => {
  // Words, not digits, because that is how the slide says it. Change
  // MAX_POPULATION and this table, and the slide text in all three languages.
  const SPELLED: Record<number, Record<string, string> | undefined> = { 2000: { nl: 'tweeduizend', en: 'two thousand', fy: 'twatûzen' } };

  const i = slides.findIndex((s) => s.route === '/simulatie');

  it('exists', () => {
    expect(i).toBeGreaterThanOrEqual(0);
  });

  it('knows how to spell the ceiling', () => {
    expect(SPELLED[MAX_POPULATION], `add MAX_POPULATION=${MAX_POPULATION} to SPELLED`).toBeDefined();
  });

  it.each(['nl', 'en', 'fy'])('names the population a run can reach, in %s', (code) => {
    const title = code === 'nl' ? slides[i]?.title : overlays[code]?.[`slides.${i}.title`];
    expect(title).toContain(SPELLED[MAX_POPULATION]?.[code]);
  });
});
