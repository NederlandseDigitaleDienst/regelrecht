/**
 * Editor BDD semantics - the single JS dispatch file (mirror of Rust dispatch.rs).
 *
 * One `dispatch(ctx, engine, action, args, table, { loadDependency })` runs the
 * effect for a canonical grammar action. The generated grammar
 * (grammar.generated.js) supplies patterns/templates; this file supplies the
 * behavior. The editor's WASM engine implements only the `core` tier; actions
 * from the `notes`/`untranslatable`/`provenance` tiers throw, so a conformance
 * feature can never silently no-op in the editor.
 *
 * `args` is the ordered capture list (typed per the grammar's argTypes) with any
 * grammar `literals` appended. `table` is the step's dataTable rows (string[][])
 * or null. Whether row 0 is a header depends on the step: a data-source table
 * has one, a parameter table does not.
 */

import { VALUE_TYPING } from './grammar.generated.js';
import { isUnknown, missingFacts } from '../values';
import type { ExecutionContext, ExecutionResult } from './context';

/**
 * The engine methods the dispatch calls. The WASM engine's `WasmEngine`
 * fits; its generated `.d.ts` lives in each app's `public/wasm/pkg`, out of
 * reach of this package, so the runner names only what it uses.
 */
export interface GherkinEngine {
  hasLaw(lawId: string): boolean;
  registerDataSource(name: string, keyField: string, records: Record<string, unknown>[]): void;
  registerDataSourceForLaw(lawId: string, name: string, keyField: string, records: Record<string, unknown>[]): void;
  execute(lawId: string, outputName: string, parameters: Record<string, unknown>, calculationDate: string): ExecutionResult;
}

export interface DispatchOptions {
  /** Fetch and load a law a scenario depends on. */
  loadDependency: (lawId: string) => Promise<void>;
}

/** A step's data table: rows of cells, as the parser gives them. */
export type DataTable = string[][];

/** Same shape as the runtime checks below; mirrors the codegen. */
type ValueTyping = { quoted: string; bare: string; table_cell: string };
const TYPING: ValueTyping = VALUE_TYPING;

/**
 * Parse a string value to a typed value, mirroring Rust value_conversion.rs
 * (`convert_gherkin_value`).
 * - "true"/"false" → boolean
 * - "null" and the empty string → null (an absence; the Rust runner reads
 *   `parameter "x" is ""` as Null, so this runner must too)
 * - numeric → number (int or float)
 * - otherwise → string
 *
 * The empty *cell* in a table never reaches here: `tableToRecords` and
 * `set_parameters_table` leave the key out first (RFC-036, "not stated").
 */
export function parseValue(str: unknown): unknown {
  // Already typed (a form hands real booleans and numbers straight through).
  if (typeof str !== 'string') return str;
  if (str === 'true') return true;
  if (str === 'false') return false;
  if (str === 'null' || str.trim() === '') return null;

  // A JSON array or object literal: the only way an array- or object-typed
  // register input fits in a table cell. Mirrors the Rust helper
  // (`convert_gherkin_value`); a cell that does not parse stays a string.
  if (str.startsWith('[') || str.startsWith('{')) {
    try {
      return JSON.parse(str);
    } catch {
      // not JSON after all: fall through to the scalar rules
    }
  }

  // Try integer first, then float
  if (/^-?\d+$/.test(str)) {
    const n = parseInt(str, 10);
    if (Number.isSafeInteger(n)) return n;
  }
  if (/^-?\d+\.\d+$/.test(str)) {
    const f = parseFloat(str);
    if (Number.isFinite(f)) return f;
  }

  return str;
}

/**
 * Type a quoted capture. The three `value_typing` rules live in
 * bdd/grammar.yaml and reach both dispatchers through codegen, so neither
 * engine can hold its own opinion about what a quote means.
 */
export function quotedValue(raw: unknown): unknown {
  switch (TYPING.quoted) {
    case 'literal': return raw;
    case 'inferred': return parseValue(raw);
    default:
      throw new Error(`bdd/grammar.yaml: unknown value_typing.quoted '${TYPING.quoted}'`);
  }
}

/** Type a bare (unquoted) capture; see [quotedValue] for where the rule lives. */
export function bareValue(raw: string): number {
  if (TYPING.bare !== 'number') {
    throw new Error(`bdd/grammar.yaml: unknown value_typing.bare '${TYPING.bare}'`);
  }
  return Number(raw);
}

/** Type a data-table cell; see [quotedValue] for where the rule lives. */
export function tableCellValue(raw: string): unknown {
  switch (TYPING.table_cell) {
    case 'inferred': return parseValue(raw);
    case 'literal': return raw.trim();
    default:
      throw new Error(`bdd/grammar.yaml: unknown value_typing.table_cell '${TYPING.table_cell}'`);
  }
}

/**
 * Parse a data table into record objects using the header row.
 *
 * An empty cell (after trimming) means the record has no value for that
 * column, so the key is left out of the record altogether (RFC-036): a data
 * source that lacks the field makes the input Unknown, with the field named
 * as the missing fact. The literal `null` in a cell stays `null`, an absence
 * the data states.
 *
 * The same rule applies to every header-row table, collection parameters
 * (`set_parameter_collection`) included, because the Rust mirror
 * (`rows_to_records`) is one function for both. An element with an omitted
 * key behaves like any object without that property; an author who means
 * "this element has no such value" writes `null`. A parameter table
 * (`set_parameters_table`) has no header row and does not pass through here,
 * but follows the same rule: an empty value cell leaves the parameter out.
 *
 * A row that does not match the header row is rejected rather than filled with
 * undefined: today the Gherkin parser already rejects a varying cell count, but
 * that guarantee lives in a dependency we bump, and the Rust mirror leans on a
 * different parser. The explicit check keeps both sides failing the same way
 * if either parser ever loosens.
 */
export function tableToRecords(dataTable: DataTable | null | undefined): Record<string, unknown>[] {
  if (!dataTable || dataTable.length < 2) return [];
  const headers = dataTable[0]!;
  return dataTable.slice(1).map((row, rowIndex) => {
    if (row.length !== headers.length) {
      throw new Error(
        `data table row ${rowIndex + 1} has ${row.length} cells, header row has ${headers.length}`,
      );
    }
    const record: Record<string, unknown> = {};
    headers.forEach((h, i) => {
      const cell = row[i]!;
      if (cell.trim() === '') return;
      record[h] = tableCellValue(cell);
    });
    return record;
  });
}

export function getOutput(ctx: ExecutionContext, name: string): unknown {
  if (!ctx.result || !ctx.result.outputs) {
    throw new Error(`No outputs available (execution ${ctx.executed ? 'failed' : 'not performed'})`);
  }
  return ctx.result.outputs[name];
}

/**
 * Structural equality between an engine output and an expected value,
 * mirroring the Rust harness (`values_equal_with_tolerance` on top of
 * `Value`'s `PartialEq` in law-model):
 * - numbers: equal within 1e-9 (the engine keeps exact decimals, the JS side
 *   sees f64, so literal-parsing noise must not fail a scenario);
 * - null only equals null (a missing output is `undefined`, not null);
 * - an Unknown value (RFC-036) equals any other Unknown value, whatever
 *   facts each lists as missing, and nothing else — not null, not an
 *   ordinary object;
 * - a number never equals its string form, a boolean never equals anything
 *   but the same boolean;
 * - arrays: same length, elements equal in order;
 * - objects: same key set, values equal per key (a BTreeMap on the Rust
 *   side, so key order is irrelevant).
 * The tolerance is applied at every depth. Rust applies it only at the top
 * level and compares nested Int/Decimal exactly, but that distinction does
 * not exist in JS (both are `number`), so a recursive tolerance is the
 * closest mapping and never rejects what Rust accepts.
 */
export function valuesEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a === null || b === null || a === undefined || b === undefined) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a === 'number') return Math.abs(a - (b as number)) < 1e-9;
  if (typeof a !== 'object' || typeof b !== 'object') return false;
  if (isUnknown(a) || isUnknown(b)) return isUnknown(a) && isUnknown(b);

  const aIsArray = Array.isArray(a);
  if (aIsArray !== Array.isArray(b)) return false;
  if (Array.isArray(a)) {
    const bList = b as unknown[];
    if (a.length !== bList.length) return false;
    return a.every((item, i) => valuesEqual(item, bList[i]));
  }

  const aRecord = a as Record<string, unknown>;
  const bRecord = b as Record<string, unknown>;
  const aKeys = Object.keys(aRecord);
  const bKeys = Object.keys(bRecord);
  if (aKeys.length !== bKeys.length) return false;
  return aKeys.every((key) => Object.hasOwn(bRecord, key) && valuesEqual(aRecord[key], bRecord[key]));
}

/** @deprecated Kept for callers of the old name; arrays and objects are compared too. */
export const primitiveEqual = valuesEqual;

function assertOutput(ctx: ExecutionContext, name: string, expected: unknown): void {
  const actual = getOutput(ctx, name);
  if (!valuesEqual(actual, expected)) {
    throw new Error(
      `Expected output "${name}" to equal ${JSON.stringify(expected)}, got: ${JSON.stringify(actual)}`,
    );
  }
}

/** `name (law)` per missing fact, for error messages. */
function describeMissing(value: unknown): string {
  return missingFacts(value).map((f) => `${f.name} (${f.law})`).join(', ');
}

function assertUnknown(ctx: ExecutionContext, name: string): unknown {
  const actual = getOutput(ctx, name);
  if (!isUnknown(actual)) {
    throw new Error(`Expected output "${name}" to be unknown, got: ${JSON.stringify(actual)}`);
  }
  return actual;
}

function tierUnsupported(tier: string): Error {
  return new Error(`tier ${tier} not supported by the editor engine`);
}

/**
 * Run a canonical grammar action against the execution context + WASM engine.
 *
 * `action` is the canonical action id from the grammar and `args` the
 * ordered typed captures plus literals. A quoted capture is typed by
 * inference (`value_typing.quoted`), so `"404"` arrives as a number; the arms
 * that need text, such as a law id or the substring of an error message,
 * read it through `text`, which turns it back into one. `table`
 * holds the step's data table rows, or null; row 0 is a header for a
 * data-source table and a value row for a parameter table.
 */
export async function dispatch(
  ctx: ExecutionContext,
  engine: GherkinEngine,
  action: string,
  args: unknown[],
  table: DataTable | null,
  { loadDependency }: DispatchOptions,
): Promise<void> {
  const text = (i: number) => String(args[i]);
  switch (action) {
    // --- core: setup ---
    case 'set_calculation_date':
      ctx.calculationDate = text(0);
      break;

    case 'load_law': {
      const lawId = text(0);
      if (!engine.hasLaw(lawId)) {
        await loadDependency(lawId);
      }
      break;
    }

    case 'set_parameter':
      // Already typed by the grammar's `value_typing` rule in buildArgs.
      ctx.parameters[text(0)] = args[1];
      break;

    // No header row on a parameter table (mirror of Rust `rows_to_params`);
    // every row is a name/value pair. An empty value cell means the parameter
    // is not passed (RFC-036): an optional one is then unknown for lack of
    // it, a required one is the caller's omission. The word `null` passes an
    // absence. The same meaning of the empty cell as in a data table.
    case 'set_parameters_table':
      for (const row of table || []) {
        const [name, value] = row;
        if (name === undefined || value === undefined || value.trim() === '') continue;
        ctx.parameters[name.trim()] = tableCellValue(value);
      }
      break;

    case 'set_parameter_collection':
      // Header row plus one row per element, each element an object keyed by
      // the column names — the shape the iterating operations expect (RFC-016).
      ctx.parameters[text(0)] = tableToRecords(table);
      break;

    case 'set_data_source': {
      const sourceName = text(0);
      const keyField = text(1);
      const records = tableToRecords(table);
      engine.registerDataSource(sourceName, keyField, records);
      break;
    }

    // Bound to one law: consulted only while that law's inputs resolve, so
    // the rows can never shadow a same-named cross-law input elsewhere.
    case 'set_data_source_for_law': {
      const sourceName = text(0);
      const keyField = text(1);
      const lawId = text(2);
      const records = tableToRecords(table);
      engine.registerDataSourceForLaw(lawId, sourceName, keyField, records);
      break;
    }

    // --- core: execute ---
    case 'evaluate': {
      const outputName = text(0);
      const lawId = text(1);
      if (!ctx.calculationDate) {
        throw new Error('No calculation date set. Add: Given the calculation date is "YYYY-MM-DD"');
      }
      try {
        ctx.result = engine.execute(lawId, outputName, ctx.parameters, ctx.calculationDate);
        ctx.executed = true;
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.executed = true;
        ctx.result = null;
      }
      break;
    }

    // --- core: asserts ---
    case 'assert_succeeds':
      if (ctx.error) {
        throw new Error(`Expected execution to succeed, but got error: ${ctx.error}`);
      }
      if (!ctx.executed) {
        throw new Error('No execution was performed');
      }
      break;

    case 'assert_fails':
      if (!ctx.error) {
        throw new Error('Expected execution to fail, but it succeeded');
      }
      break;

    case 'assert_fails_with': {
      if (!ctx.error) {
        throw new Error('Expected execution to fail, but it succeeded');
      }
      const expected = text(0);
      const errorStr = String(ctx.error);
      if (!errorStr.toLowerCase().includes(expected.toLowerCase())) {
        throw new Error(`Expected error containing "${expected}", got: ${errorStr}`);
      }
      break;
    }

    case 'assert_boolean':
      // args = [output, literalBool]
      assertOutput(ctx, text(0), args[1]);
      break;

    case 'assert_equals':
      // Numeric form arrives as Number; string form as String.
      assertOutput(ctx, text(0), args[1]);
      break;

    case 'assert_null':
      assertOutput(ctx, text(0), null);
      break;

    // Unknown (RFC-036): the output is the `{__unknown: true, missing: [...]}`
    // value, whatever it lists as missing.
    case 'assert_unknown':
      assertUnknown(ctx, text(0));
      break;

    // ... and one of the facts it lists as missing has this name.
    case 'assert_unknown_for': {
      const name = text(0);
      const fact = text(1);
      const actual = assertUnknown(ctx, name);
      if (!missingFacts(actual).some((f) => f.name === fact)) {
        throw new Error(
          `Expected output "${name}" to be unknown for lack of "${fact}", but it is unknown for lack of: ${describeMissing(actual)}`,
        );
      }
      break;
    }

    case 'assert_contains': {
      const name = text(0);
      const substring = text(1);
      const actual = getOutput(ctx, name);
      if (typeof actual !== 'string' || !actual.toLowerCase().includes(substring.toLowerCase())) {
        throw new Error(
          `Expected output "${name}" to contain "${substring}", got: ${JSON.stringify(actual)}`,
        );
      }
      break;
    }

    // --- non-core tiers: not supported by the editor engine ---
    case 'evaluate_outputs':
    case 'assert_exact_outputs':
    case 'assert_provenance':
      throw tierUnsupported('provenance');

    case 'set_findings_table':
      throw tierUnsupported('findings');

    case 'set_untranslatable_mode':
    case 'assert_tainted':
      throw tierUnsupported('untranslatable');

    case 'set_note_articles':
    case 'set_note_selector_exact':
    case 'set_note_selector_context':
    case 'set_note_hint_article':
    case 'set_note_hint_position':
    case 'resolve_note':
    case 'assert_note_resolves':
    case 'assert_note_exact_match':
    case 'assert_note_fuzzy_match':
    case 'assert_note_orphaned':
    case 'assert_note_ambiguous':
      throw tierUnsupported('notes');

    default:
      throw new Error(`unknown action '${action}' - grammar/dispatch out of sync`);
  }
}

/**
 * The root step of a trace document (RFC-039).
 *
 * `executeWithTrace` and `executeMultipleWithTrace` hand back
 * `{trace_version, root}`, the shape `schema/trace/v1/trace-schema.json`
 * defines. Consumers walk steps, so the document is opened here rather than at
 * every call site. A bare step passes through unchanged, so a trace recorded
 * before the envelope still renders.
 */
export function traceRoot<T>(trace: { root?: T } | T | null | undefined): T | null {
  if (!trace) return null;
  return (trace as { root?: T }).root ?? (trace as T);
}
