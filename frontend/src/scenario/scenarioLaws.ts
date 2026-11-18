import type { FormState } from '../gherkin/formMapper';

/**
 * The regelwerken a feature's scenarios need in the engine: what they load
 * (`Given law ... is loaded`, in the background or a scenario) and what they
 * execute.
 */
export function scenarioLawsOf(state: FormState | null): string[] {
  if (!state) return [];
  const laws = new Set<string>(state.background?.dependencies ?? []);
  for (const scenario of state.scenarios) {
    for (const dep of scenario.setup.dependencies) laws.add(dep);
    if (scenario.execution?.lawId) laws.add(scenario.execution.lawId);
  }
  return [...laws];
}
