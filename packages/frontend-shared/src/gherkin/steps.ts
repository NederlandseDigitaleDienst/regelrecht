/**
 * Gherkin step executor for the RegelRecht editor engine.
 *
 * Step definitions are derived from the generated canonical grammar
 * (grammar.generated.js) - never hand-listed here. Each definition is
 * { pattern: RegExp, tier: string, execute: async (ctx, engine, match, step) => void }.
 * The actual semantics live in actions.ts (the single dispatch file). The editor
 * supports only the `core` tier; non-core steps throw via their action arm.
 */

import { GRAMMAR } from './grammar.generated.js';
import { dispatch, bareValue, quotedValue, type GherkinEngine, type DispatchOptions } from './actions';
import type { ExecutionContext } from './context';
import type { Step } from './parser';

/**
 * One entry of the generated grammar. grammar.generated.js is plain JS
 * written by bdd/codegen/gen-js.mjs, so this is the shape it is checked
 * against here rather than a type the generator emits.
 */
export interface GrammarEntry {
  id: string;
  action: string;
  keyword: string;
  tier: string;
  datatable: boolean;
  pattern: RegExp;
  argTypes: string[];
  literals: unknown[];
  template: (args: string[]) => string;
}

const ENTRIES: readonly GrammarEntry[] = GRAMMAR;

/** A step definition: a pattern and what running a matching step does. */
export interface StepDefinition {
  pattern: RegExp;
  tier: string;
  execute: (ctx: ExecutionContext, engine: GherkinEngine, match: RegExpExecArray, step?: Step | null) => Promise<void>;
}

/** Tiers the editor's WASM engine can execute. */
export const SUPPORTED_TIERS: readonly string[] = ['core'];

/**
 * Find the grammar entry a step text belongs to.
 *
 * The one walk over the grammar, shared. Every runner that is handed raw step
 * text has to do this, and it was hand-rolled identically in three places (the
 * demo's Dutch renderer, and both panels on the docs site) before it lived
 * here. A fourth copy is a fourth chance for one of them to match differently
 * from the Rust harness.
 *
 * `text` is the step without its keyword.
 */
export function matchStep(text: string): { entry: GrammarEntry; args: string[] } | null {
  for (const entry of ENTRIES) {
    const m = entry.pattern.exec(text);
    if (m) return { entry, args: m.slice(1) };
  }
  return null;
}

/**
 * Parse a step's captures into typed args.
 *
 * Which capture becomes what is `value_typing` in bdd/grammar.yaml; the Rust
 * dispatcher reads the same three keys. The grammar literals are not appended
 * here, because a caller that only wants to *show* a step wants the captures
 * alone; `buildArgs` adds them for the callers that dispatch.
 *
 * `captures` are the captures, in order, as matchStep returns them.
 */
export function typedArgs(entry: GrammarEntry, captures: readonly string[]): unknown[] {
  return entry.argTypes.map((t, i) =>
    t === 'number' ? bareValue(captures[i]!) : quotedValue(captures[i]!),
  );
}

/** Typed captures plus the entry's literals: what `dispatch` takes. */
function buildArgs(entry: GrammarEntry, match: RegExpExecArray): unknown[] {
  return [...typedArgs(entry, match.slice(1)), ...entry.literals];
}

/**
 * Create the step definitions registry from the canonical grammar.
 *
 * `loadDependency` fetches and loads a law a scenario depends on.
 */
export function createStepDefinitions({ loadDependency }: DispatchOptions): StepDefinition[] {
  return ENTRIES.map((entry) => ({
    pattern: entry.pattern,
    tier: entry.tier,
    execute: async (ctx, engine, match, step) => {
      const args = buildArgs(entry, match);
      const table = step?.dataTable ?? null;
      await dispatch(ctx, engine, entry.action, args, table, { loadDependency });
    },
  }));
}

// parseValue has a single home in actions.ts; re-exported for the tests that
// pin which literals it recognises.
export { parseValue } from './actions';
