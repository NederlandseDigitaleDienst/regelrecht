/**
 * The edits the Machine pane and its sheets make to an article's
 * machine_readable, as pure functions on a draft the caller owns.
 *
 * `useOpenLaw` hands each of these a fresh copy of the current section and
 * stores the result, so none of them has to clone or worry about reactivity.
 */
import { withDecisionField } from '@regelrecht/frontend-shared/produces';
import type { EditSheetSave } from '../components/EditSheet.vue';
import type { ExecutionAction, ExecutionField, ExecutionSection, MachineReadableSection } from './lawFormat';

/** A save from EditSheet, or a field patch from MachineReadable (`section: 'produces'`). */
export type MachineReadableSave = EditSheetSave | { section: 'produces'; key: string; data: string | null };

/** A delete from MachineReadable: a definition by name, or a row by index. */
export type MachineReadableDelete =
  | { section: 'definition'; key: string }
  | { section: 'parameter' | 'input' | 'output' | 'action'; index: number };

/** An execution section with every list the edits below write to. */
type FilledExecution = ExecutionSection & { parameters: ExecutionField[]; input: ExecutionField[]; output: ExecutionField[] };

/** Make sure the lists a save writes to exist. */
function filledExecution(mr: MachineReadableSection): FilledExecution {
  if (!mr.definitions) mr.definitions = {};
  const execution = mr.execution ?? {};
  execution.parameters ??= [];
  execution.input ??= [];
  execution.output ??= [];
  mr.execution = execution;
  return execution as FilledExecution;
}

/**
 * Apply a save from the Machine pane or its edit sheet to `mr`.
 *
 * EditSheet types `key` and `index` as optional because its item type is
 * shared by every section; an existing definition or row always arrives with
 * them, hence the casts below.
 */
export function applySave(mr: MachineReadableSection, payload: MachineReadableSave): void {
  const execution = filledExecution(mr);
  // filledExecution set it.
  const definitions = mr.definitions as Record<string, unknown>;
  switch (payload.section) {
    case 'definition':
      if (payload.newKey && payload.newKey !== payload.key) delete definitions[payload.key as string];
      definitions[(payload.newKey || payload.key) as string] = payload.data;
      break;
    case 'add-definition':
      definitions[payload.key] = payload.data;
      break;
    case 'parameter':
      execution.parameters[payload.index as number] = payload.data;
      break;
    case 'add-parameter':
      execution.parameters.push(payload.data);
      break;
    case 'input':
      execution.input[payload.index as number] = payload.data;
      break;
    case 'add-input':
      execution.input.push(payload.data);
      break;
    case 'output':
      execution.output[payload.index as number] = payload.data;
      break;
    case 'add-output':
      execution.output.push(payload.data);
      break;
    case 'produces': {
      const produces = withDecisionField(execution.produces, payload.key, payload.data);
      if (produces === undefined) delete execution.produces;
      else execution.produces = produces;
      break;
    }
  }
}

/**
 * Remove an item from `mr`. Out-of-range indices and missing keys do nothing,
 * so a stale event from the UI can never crash.
 */
export function applyDelete(mr: MachineReadableSection, payload: MachineReadableDelete): void {
  if (payload.section === 'definition') {
    if (mr.definitions && payload.key in mr.definitions) delete mr.definitions[payload.key];
    return;
  }
  const list = {
    parameter: mr.execution?.parameters,
    input: mr.execution?.input,
    output: mr.execution?.output,
    action: mr.execution?.actions,
  }[payload.section];
  if (list && payload.index >= 0 && payload.index < list.length) list.splice(payload.index, 1);
}

/** The scaffold a new machine_readable starts from. */
export function emptyMachineReadable(): MachineReadableSection {
  return {
    definitions: {},
    execution: { parameters: [], input: [], output: [], actions: [] },
  };
}

/**
 * A new action, seeded with an EQUALS stub instead of an empty literal so the
 * action sheet has an operation tree to render and the user can reach the
 * type dropdown right away. The completeness check rejects the unfilled stub
 * on save.
 */
export function newActionStub(): ExecutionAction {
  return { output: '', value: { operation: 'EQUALS', subject: '', value: '' } };
}
