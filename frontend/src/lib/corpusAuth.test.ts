import { describe, it, expect } from 'vitest';
import { deriveAuthRef, tokenEnvName } from './corpusAuth';

// The fixtures below are the ones from the Rust unit tests in
// packages/editor-api/src/trajects.rs (auth_ref_*). They are copied on purpose:
// this file exists to catch the frontend drifting away from the backend.
describe('deriveAuthRef', () => {
  it('lowercases and collapses separator runs', () => {
    expect(deriveAuthRef('Acme', 'Secret-Repo')).toBe('acme-secret-repo');
    expect(deriveAuthRef('MinBZK', 'regelrecht-corpus')).toBe('minbzk-regelrecht-corpus');
    expect(deriveAuthRef('a.b', 'c_d')).toBe('a-b-c-d');
  });

  it('leaves already normalised input untouched', () => {
    expect(deriveAuthRef('acme', 'secret-repo')).toBe('acme-secret-repo');
    expect(deriveAuthRef('minbzk', 'regelrecht-corpus')).toBe('minbzk-regelrecht-corpus');
  });

  it('trims leading and trailing dashes', () => {
    expect(deriveAuthRef('...', '...')).toBe('');
    expect(deriveAuthRef('.foo.', '.bar.')).toBe('foo-bar');
  });
});

describe('tokenEnvName', () => {
  it('uppercases the ref and turns dashes into underscores', () => {
    expect(tokenEnvName('acme-secret-repo')).toBe('CORPUS_AUTH_ACME_SECRET_REPO_TOKEN');
    expect(tokenEnvName('minbzk-regelrecht-corpus')).toBe('CORPUS_AUTH_MINBZK_REGELRECHT_CORPUS_TOKEN');
  });
});
