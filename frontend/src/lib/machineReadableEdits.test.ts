import { describe, expect, it } from 'vitest';
import { applyDelete, applySave } from './machineReadableEdits';
import type { MachineReadableSection } from './lawFormat';

function section(): MachineReadableSection {
  return {
    definitions: { drempel: 100 },
    execution: {
      parameters: [{ name: 'bsn' }],
      input: [{ name: 'inkomen' }],
      output: [{ name: 'hoogte' }],
      actions: [{ output: 'hoogte', value: 1 }],
    },
  };
}

describe('applySave', () => {
  it('renames a definition and keeps its value', () => {
    const mr = section();
    applySave(mr, { section: 'definition', key: 'drempel', newKey: 'grens', data: 150 });
    expect(mr.definitions).toEqual({ grens: 150 });
  });

  it('replaces and appends rows', () => {
    const mr = section();
    applySave(mr, { section: 'input', index: 0, data: { name: 'vermogen' } });
    applySave(mr, { section: 'add-output', data: { name: 'recht' } });
    expect(mr.execution?.input).toEqual([{ name: 'vermogen' }]);
    expect(mr.execution?.output).toEqual([{ name: 'hoogte' }, { name: 'recht' }]);
  });

  it('creates the lists a save writes to on an empty section', () => {
    const mr: MachineReadableSection = {};
    applySave(mr, { section: 'add-parameter', data: { name: 'bsn' } });
    expect(mr.definitions).toEqual({});
    expect(mr.execution?.parameters).toEqual([{ name: 'bsn' }]);
  });
});

describe('applyDelete', () => {
  it('removes a definition by name and a row by index', () => {
    const mr = section();
    applyDelete(mr, { section: 'definition', key: 'drempel' });
    applyDelete(mr, { section: 'action', index: 0 });
    expect(mr.definitions).toEqual({});
    expect(mr.execution?.actions).toEqual([]);
  });

  it('ignores a stale index or a missing key', () => {
    const mr = section();
    applyDelete(mr, { section: 'parameter', index: 5 });
    applyDelete(mr, { section: 'definition', key: 'onbekend' });
    expect(mr).toEqual(section());
  });
});
