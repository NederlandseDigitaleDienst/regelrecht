// apiFetch now lives in the shared package (@regelrecht/frontend-shared) so the
// editor, admin and lawmaking share one implementation. This thin re-export
// keeps the editor's ~25 call sites importing from './lib/apiFetch'
// unchanged. The 401 redirect still lives in ./apiAuthGuard (app-local).
export { apiFetch, apiFetchJson, apiFetchText, ApiError } from '@regelrecht/frontend-shared';

/**
 * An error from a request: an `ApiError` carries the HTTP `status`, while a
 * network failure is a plain `Error` without one.
 */
export type RequestError = Error & { status?: number };
