/**
 * Types for a parsed rulework (the law format), as far as the editor reads it.
 *
 * The canonical contract is `schema/latest/schema.json`; these types describe
 * the parts the editor uses and keep the format's own keys. Nothing validates
 * a parsed YAML against them: they say what the code expects, and every
 * object keeps an index signature for the fields not listed here.
 *
 * Components that read only a few fields declare their own small structural
 * type; a `LawArticle` is assignable to each of them.
 */
import type { Produces } from '@regelrecht/frontend-shared/produces';

/** Where an input gets its value: another regulation's output, or a data source. */
export interface InputSource {
  regulation?: string;
  output?: string;
  parameters?: Record<string, unknown> | null;
  [key: string]: unknown;
}

/** A parameter, input or output entry of `machine_readable.execution`. */
export interface ExecutionField {
  name: string;
  type?: string;
  required?: boolean;
  type_spec?: { unit?: string | null; [key: string]: unknown } | null;
  // Only `true` counts; the YAML is unchecked, so anything else may be here.
  nullable?: unknown;
  source?: InputSource | null;
  [key: string]: unknown;
}

/** An action: the operation tree that computes one output. */
export interface ExecutionAction {
  output?: string;
  value?: unknown;
  [key: string]: unknown;
}

/** An article's `machine_readable.execution` section. */
export interface ExecutionSection {
  produces?: Produces;
  parameters?: ExecutionField[] | null;
  input?: ExecutionField[] | null;
  output?: ExecutionField[] | null;
  actions?: ExecutionAction[] | null;
  [key: string]: unknown;
}

/**
 * A definition as written in `machine_readable.definitions`: a bare value (a
 * scalar or a list) or `{ value, type_spec }`.
 */
export type RawDefinition =
  | string
  | number
  | boolean
  | unknown[]
  | { value?: unknown; type_spec?: { unit?: string } | null; [key: string]: unknown };

/** Whether a value from `definitions` is one of the shapes a `RawDefinition` can take. */
export function isRawDefinition(value: unknown): value is RawDefinition {
  return ['string', 'number', 'boolean'].includes(typeof value) || (value !== null && typeof value === 'object');
}

/** An article's `machine_readable` section. */
export interface MachineReadableSection {
  /** Each definition is a scalar, a list, or an object with `value`. */
  definitions?: Record<string, unknown> | null;
  execution?: ExecutionSection | null;
  [key: string]: unknown;
}

/** One article of a rulework. */
export interface LawArticle {
  /** A string in most files; unquoted YAML parses it as a number. */
  number: string | number;
  text?: string;
  machine_readable?: MachineReadableSection | null;
  [key: string]: unknown;
}

/** A parsed rulework: one version of a regulation. */
export interface LawDocument {
  $id?: string;
  name?: string;
  articles?: LawArticle[];
  [key: string]: unknown;
}
