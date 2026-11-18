// useAuth now lives in the shared package (@regelrecht/frontend-shared) so the
// editor and admin share one implementation. This thin re-export keeps the
// editor's call sites (AppShell, router guard, apiAuthGuard) importing from
// './composables/useAuth.js' unchanged.
export { useAuth, ensureAuthReady, hasRole, hasAnyRole } from '@regelrecht/frontend-shared';
