// useGithubAuth lives in the shared package (@regelrecht/frontend-shared) so
// the editor and admin share one implementation. This thin re-export keeps the
// editor's call sites importing from './composables/useGithubAuth.js'.
export { useGithubAuth, ensureGithubReady } from '@regelrecht/frontend-shared';
