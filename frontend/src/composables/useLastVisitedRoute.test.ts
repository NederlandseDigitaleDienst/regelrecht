import { describe, it, expect } from 'vitest';
import type { Router } from 'vue-router';
import { trajectSwitchTarget, sectionTarget, editorTabTarget } from './useLastVisitedRoute';

const REF = 'ander-traject-12345678';
const ACTIVE = 'actief-traject-0a1b2c3d';

// Minimal fake router: `resolve(path)` returns a canned resolution keyed on the
// exact path, so sectionTarget's branch logic can be exercised without a real
// router instance.
// The canned resolutions carry only name/params/query, so the fake is cast to
// the one router method sectionTarget uses.
function fakeRouter(map: Record<string, { name: string; params: Record<string, string>; query: Record<string, string> }>) {
  return { resolve: (path: string) => map[path] } as unknown as Pick<Router, 'resolve'>;
}

describe('trajectSwitchTarget', () => {
  it('lands the editor on the bare traject root, dropping the open law', () => {
    // A switch from the editor must not carry the law/article across: the new
    // traject has its own corpus, so the old lawId would point at a document it
    // doesn't have. We navigate to the traject-scoped editor root instead.
    const target = trajectSwitchTarget('editor-traject', REF);
    expect(target).toEqual({ name: 'editor-traject', params: { trajectRef: REF } });
    expect(target.params.lawId).toBeUndefined();
    expect(target.params.articleNumber).toBeUndefined();
  });

  it('lands Home on the bare traject home, dropping the open law', () => {
    // Same rule for the bibliotheek side: go to the traject home root, not the
    // corpus view of a law the new traject may not have.
    const target = trajectSwitchTarget('library-traject', REF);
    expect(target).toEqual({ name: 'traject-home', params: { trajectRef: REF } });
  });

  it('treats every Home-section route name as Home', () => {
    for (const name of ['traject-home', 'corpus-juris', 'werkdocumenten-traject', 'taken-traject']) {
      const target = trajectSwitchTarget(name, REF);
      expect(target.name).toBe('traject-home');
      expect(target.params).toEqual({ trajectRef: REF });
    }
  });
});

describe('sectionTarget (Editor tab)', () => {
  it('drops a foreign lawId on a cross-traject editor path', () => {
    // The stored editor path belongs to a DIFFERENT traject: its lawId may not
    // exist in the active one, so we must NOT stamp it onto the active traject.
    // Land on the active traject's editor root and let restore take over.
    const stored = `/trajecten/${REF}/editor/wet_x/2`;
    const router = fakeRouter({
      [stored]: {
        name: 'editor-traject',
        params: { trajectRef: REF, lawId: 'wet_x', articleNumber: '2' },
        query: {},
      },
    });
    const target = sectionTarget(router, stored, ACTIVE);
    expect(target).toEqual({ name: 'editor-traject', params: { trajectRef: ACTIVE } });
    expect(target.params.lawId).toBeUndefined();
  });

  it('returns the stored path verbatim when its traject matches the active one', () => {
    // Same traject: the Editor tab keeps the exact law/article (and any
    // query/hash) - the whole point of remembering where the user was.
    const stored = `/trajecten/${ACTIVE}/editor/wet_x/2`;
    const router = fakeRouter({
      [stored]: {
        name: 'editor-traject',
        params: { trajectRef: ACTIVE, lawId: 'wet_x', articleNumber: '2' },
        query: {},
      },
    });
    expect(editorTabTarget(router, stored, ACTIVE)).toBe(stored);
  });

  it('keeps the chooser `?law=` intact, upgrading it onto the active traject', () => {
    // A traject-less chooser path carries an UNSCOPED intended law (not a law
    // from another traject), so it is stamped onto the active traject to open.
    const stored = '/editor?law=wet_x';
    const router = fakeRouter({
      [stored]: { name: 'editor', params: {}, query: { law: 'wet_x' } },
    });
    const target = sectionTarget(router, stored, ACTIVE);
    expect(target).toEqual({
      name: 'editor-traject',
      params: { trajectRef: ACTIVE, lawId: 'wet_x' },
    });
  });
});
