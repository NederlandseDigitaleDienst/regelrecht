// @regelrecht/frontend-shared — shared frontend primitives consumed by the
// editor (frontend/, including its harvester "Beheer" section under
// frontend/src/harvester/), lawmaking (frontend-lawmaking/), the demo
// (frontend-demo/) and the OCW-pocs (frontend-poc-*). The editor is the
// canonical source for what lives here; other apps conform to it.
//
// Not everything hangs off this index. The Gherkin runner, the separate lib
// modules and the shared components have their own subpath in `exports`
// (./gherkin, ./lib/*, ./components/*, ./usePersistedState), so an app pulls
// in only what it uses.
export { apiFetch, apiFetchJson, apiFetchText, ApiError } from './apiFetch';
export type { ApiFetchInit } from './apiFetch';
export { useAuth, ensureAuthReady, hasRole, hasAnyRole } from './useAuth';
export type { AuthPerson, AuthStatus } from './useAuth';
export { useGithubAuth, ensureGithubReady } from './useGithubAuth';
export type { GithubStatus } from './useGithubAuth';
// `applyColorScheme`, `createLocalStoragePersistence` and `VALID_THEMES` stay
// internal to useColorScheme — they're implementation details, not part of the
// package's public surface (no consumer imports them).
export { useColorScheme } from './useColorScheme';
export type { ColorSchemePersistence, Theme } from './useColorScheme';
// Engine value helpers (RFC-036): one definition of the Unknown value shape
// for the editor, the demo and the Gherkin runner.
export { isUnknown, missingFacts } from './values';
export type { MissingFact, MissingKind, UnknownValue } from './values';
// What an article produces, in the object form or as a list (schema v0.8.1).
export {
  SUBMISSION_KINDS,
  decisionOf,
  isListItem,
  legalCharacterOf,
  producedDecision,
  producedItems,
  producedSubmission,
  submissionKindOf,
  withDecisionField,
} from './produces';
export type { Decision, ProducedItem, Produces } from './produces';
