// Moved to @regelrecht/frontend-shared. This shim sets the case name that
// decides the key prefix, and keeps this app's imports working.
//
// The case is set here and not in the shared file because the two OCW PoCs
// run hosted behind the same portal on the same origin; without a prefix of
// their own they would read each other's choices.
import { setCase } from '@regelrecht/frontend-shared/usePersistedState';

setCase('terugbetaalregimes');

export {
  persistedRef,
  readPersisted,
  writePersisted,
  protectKey,
  resetPersistedState,
  hasPersistedState,
} from '@regelrecht/frontend-shared/usePersistedState';
