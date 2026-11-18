import type { DemoProfileConfig } from './loadCorpus';

/**
 * Is a flag on? What the presenter toggled (`overrides`) wins over the
 * profile; what they left alone follows `demo-config.yaml`. A flag the profile
 * does not mention is off.
 */
export function featureFlagEnabled(
  profile: Pick<DemoProfileConfig, 'feature_flags'> | null | undefined,
  overrides: Partial<Record<string, boolean>> | null | undefined,
  key: string,
): boolean {
  const override = overrides?.[key];
  if (override !== undefined) return override;
  return !!profile?.feature_flags?.[key];
}
