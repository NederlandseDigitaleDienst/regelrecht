import { mount } from '@vue/test-utils';
import ScenarioForm, { type ScenarioDrill } from '../components/ScenarioForm.vue';
import { valuesFromSetup, type ScenarioValues } from '../scenario/inputs';
import type { FormAssertion, FormParameter, FormSetup, StateDataSource } from '../gherkin/formMapper';

type FieldMetaMap = Map<string, { type: string; unit: string | null; nullable?: boolean }>;

export interface ScenarioFormOptions {
  parameters?: FormParameter[];
  dataSources?: StateDataSource[];
  calculationDate?: string | null;
  assertions?: FormAssertion[];
  typeMap?: FieldMetaMap | null;
  externalFieldTypeMap?: FieldMetaMap | null;
  showResultButton?: boolean;
}

/**
 * Mount a ScenarioForm the way the open feature file drives it: the values and
 * the drill are the caller's, and every update is fed back as the new prop.
 * `values()` reads what the caller holds now.
 */
export function mountScenarioForm(options: ScenarioFormOptions = {}) {
  const setup: FormSetup = {
    calculationDate: options.calculationDate === undefined ? '2025-01-01' : options.calculationDate,
    dependencies: [],
    parameters: options.parameters ?? [],
    dataSources: options.dataSources ?? [],
  };
  let current: ScenarioValues = valuesFromSetup(setup);
  const wrapper = mount(ScenarioForm, {
    props: {
      scenario: { assertions: options.assertions ?? [] },
      values: current,
      drill: null as ScenarioDrill,
      typeMap: options.typeMap ?? null,
      externalFieldTypeMap: options.externalFieldTypeMap ?? null,
      showResultButton: options.showResultButton ?? false,
      'onUpdate:values': (next: ScenarioValues) => {
        current = next;
        void wrapper.setProps({ values: next });
      },
      'onUpdate:drill': (drill: ScenarioDrill) => {
        void wrapper.setProps({ drill });
      },
    },
  });
  return Object.assign(wrapper, {
    /** The values the caller holds now. */
    values: () => current,
    /** Replace the values from outside, as a discard does. */
    replaceValues: async (next: ScenarioValues) => {
      current = next;
      await wrapper.setProps({ values: next });
    },
  });
}

export type ScenarioFormWrapper = ReturnType<typeof mountScenarioForm>;
