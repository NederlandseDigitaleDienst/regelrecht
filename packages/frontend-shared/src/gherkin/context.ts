/** What `engine.execute` returns, as far as the runner reads it. */
export interface ExecutionResult {
  outputs?: Record<string, unknown>;
  [field: string]: unknown;
}

/**
 * ExecutionContext - holds state for a single scenario execution.
 */
export class ExecutionContext {
  calculationDate: string | null = null;
  parameters: Record<string, unknown> = {};
  result: ExecutionResult | null = null;
  /** Whatever the engine threw; the WASM engine throws strings as well as errors. */
  error: unknown = null;
  executed = false;
}
