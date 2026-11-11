// Feature-flag state for a testing Pinia's `initialState`.
import { DEFAULTS } from '../stores/featureFlags';

/** Every known feature flag switched on, as `initialState` for the featureFlags store. */
export function allFlagsOn(): { flags: Record<string, boolean> } {
  return { flags: Object.fromEntries(Object.keys(DEFAULTS).map((key) => [key, true])) };
}
