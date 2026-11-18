// The LLM providers an enrichment can run on, in menu order.
// `id` is the name the pipeline uses (ENRICH_PROVIDERS in
// packages/pipeline/src/enrich.rs); the editor-api rejects any other. `label`
// is what a user sees: `opencode` is the CLI that runs it, VLAM is the service
// it talks to. `model` is the model a new run uses; it stays out of `label`,
// because earlier results under the same id came from a different model.
export interface EnrichProviderOption {
  id: string;
  label: string;
  model?: string;
}

export const ENRICH_PROVIDER_OPTIONS: readonly EnrichProviderOption[] = [
  { id: 'claude', label: 'Claude' },
  { id: 'opencode', label: 'VLAM', model: 'Kimi K3' },
];

/** The name to show for a provider id; an unknown id is shown as it is. */
export function providerLabel(id: string): string {
  return ENRICH_PROVIDER_OPTIONS.find((o) => o.id === id)?.label ?? id;
}
