import { describe, it, expect } from 'vitest';
import { GRAMMAR } from './grammar.generated';
import type { GrammarEntry } from './steps';
import { parseFeature } from './parser';
import { mapFeatureToForm, formStateToGherkin } from './formMapper';

/** `value`, after asserting it is there: a missing value fails the test at this point. */
function defined<T>(value: T | null | undefined): T {
  expect(value).toBeDefined();
  expect(value).not.toBeNull();
  return value as T;
}

// The mapper consumes the `core` tier of the grammar. A core entry it does
// not extract falls through to unmatchedSteps: the step is written back on
// save, but the editor runs the scenario without it and says nothing. That
// is how `set_parameter_collection` (RFC-016) shipped in the grammar, the
// Rust runner and actions.js while the editor kept dropping it. This test
// synthesizes one step per core entry from its own template and demands
// that the mapper recognizes each, so a grammar extension without a mapper
// case fails here instead of in a colleague's editor.
const CORE: GrammarEntry[] = GRAMMAR.filter((e) => e.tier === 'core');

const sampleArg = (type: string, i: number) => (type === 'number' ? String(10 + i) : `arg${i}`);

function stepFor(entry: GrammarEntry): string {
  const args = entry.argTypes.map(sampleArg);
  const keyword = entry.keyword.charAt(0).toUpperCase() + entry.keyword.slice(1);
  const lines = [`    ${keyword} ${entry.template(args)}`];
  if (entry.datatable) {
    // Two columns, one data row - a shape every table-taking step accepts.
    lines.push('      | arg1 | col |');
    lines.push('      | 1    | x   |');
  }
  return lines.join('\n');
}

describe('formMapper covers every core grammar entry', () => {
  it.each(CORE.map((e) => [e.id, e]))('recognizes %s', (_id, entry) => {
    const form = mapFeatureToForm(parseFeature(`Feature: Coverage\n\n  Scenario: One step\n${stepFor(entry)}\n`));
    expect(defined(form.scenarios[0]).unmatchedSteps.map((s) => s.text)).toEqual([]);
  });

  it('writes every core entry back out as a step the mapper recognizes again', () => {
    const text = `Feature: Coverage\n\n  Scenario: All steps\n${CORE.map(stepFor).join('\n')}\n`;
    const form = mapFeatureToForm(parseFeature(text));
    const again = mapFeatureToForm(parseFeature(formStateToGherkin(form)));
    expect(defined(again.scenarios[0]).unmatchedSteps).toEqual([]);
    // The round trip preserves what the form carries: the setup, the
    // execution and the assertions.
    expect(defined(again.scenarios[0]).setup).toEqual(defined(form.scenarios[0]).setup);
    expect(defined(again.scenarios[0]).execution).toEqual(defined(form.scenarios[0]).execution);
    expect(defined(again.scenarios[0]).assertions).toEqual(defined(form.scenarios[0]).assertions);
  });
});
