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
  /**
   * Models a single run may ask for (ENRICH_MODELS in
   * packages/pipeline/src/enrich.rs, which the editor-api checks against).
   * Empty: the model comes from the deployment and is not chosen per run.
   */
  models?: readonly { id: string; label: string }[];
}

export const ENRICH_PROVIDER_OPTIONS: readonly EnrichProviderOption[] = [
  {
    id: 'claude',
    label: 'Claude',
    models: [
      { id: 'claude-opus-5-5', label: 'Opus 5.5' },
      { id: 'claude-sonnet-5-5', label: 'Sonnet 5.5' },
    ],
  },
  { id: 'opencode', label: 'VLAM', model: 'Kimi K3' },
];

/** The models a provider offers per run; empty when the deployment decides. */
export function providerModels(id: string): readonly { id: string; label: string }[] {
  return ENRICH_PROVIDER_OPTIONS.find((o) => o.id === id)?.models ?? [];
}

/** The name to show for a provider id; an unknown id is shown as it is. */
export function providerLabel(id: string): string {
  return ENRICH_PROVIDER_OPTIONS.find((o) => o.id === id)?.label ?? id;
}
