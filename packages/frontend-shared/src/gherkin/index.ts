// Engine-agnostic Gherkin runner for the canonical BDD grammar
// (bdd/grammar.yaml): parser, generated step patterns, and the single JS
// dispatch that gives them meaning against a WasmEngine. Shared by the editor
// (frontend/) and the demo (frontend-demo/).
export { parseFeature } from './parser';
export type { Step, Scenario, ParsedFeature } from './parser';
export { GRAMMAR, VALUE_TYPING } from './grammar.generated.js';
export { createStepDefinitions, SUPPORTED_TIERS, matchStep, typedArgs } from './steps';
export type { GrammarEntry, StepDefinition } from './steps';
export {
  dispatch,
  parseValue,
  quotedValue,
  bareValue,
  tableCellValue,
  tableToRecords,
  traceRoot,
  getOutput,
  valuesEqual,
  primitiveEqual,
} from './actions';
export type { GherkinEngine, DispatchOptions, DataTable } from './actions';
export { isUnknown, missingFacts } from '../values';
export { ExecutionContext } from './context';
export type { ExecutionResult } from './context';
