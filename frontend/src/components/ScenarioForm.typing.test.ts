import { describe, it, expect } from 'vitest';
import ScenarioParameterInput from './ScenarioParameterInput.vue';
import type { FormParameter } from '../gherkin/formMapper';
import { mountScenarioForm, type ScenarioFormWrapper } from '../test-utils/scenarioForm';

// A parameter the law declares arrives typed from its control, and a string
// from a text control is text: digits in a field declared `string` are saved
// as `is "…"`, which is what the engine takes for that field (RFC-055). A
// parameter without a declaration has only a text field, so what is typed
// there is read by its content.

const typeMap = new Map([['bsn', { type: 'string', unit: null }]]);

const mountForm = (parameters: FormParameter[]) =>
  mountScenarioForm({ parameters, dataSources: [], typeMap, externalFieldTypeMap: null });

const type = async (w: ScenarioFormWrapper, name: string, value: unknown) => {
  const input = w.findAllComponents(ScenarioParameterInput).find((c) => c.props('name') === name);
  if (!input) throw new Error(`no parameter input ${name}`);
  input.vm.$emit('update', value);
  await w.vm.$nextTick();
  return w.values().parameterValues[name];
};

describe('ScenarioForm types what is typed', () => {
  it('keeps digits as text in a parameter declared a string, also when the file had a number', async () => {
    const w = mountForm([{ name: 'bsn', value: 999993653 }]);
    expect(await type(w, 'bsn', '999993653')).toBe('999993653');
  });

  it('reads digits as a number in a parameter without a declaration', async () => {
    const w = mountForm([{ name: 'inkomen', value: 30000 }]);
    expect(await type(w, 'inkomen', '45000')).toBe(45000);
  });

  it('leaves other text in a parameter without a declaration as it is', async () => {
    const w = mountForm([{ name: 'gemeente', value: 'GM0384' }]);
    expect(await type(w, 'gemeente', 'GM0363')).toBe('GM0363');
    expect(await type(w, 'gemeente', 'null')).toBe('null');
  });
});
