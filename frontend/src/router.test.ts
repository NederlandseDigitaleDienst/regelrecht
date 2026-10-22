import { describe, it, expect } from 'vitest';
import type { RouteLocationMatched } from 'vue-router';
import router from './router';
import {
  sectionTarget,
  trajectTabTarget,
  isCorpusSection,
  isTrajectSection,
} from './composables/useLastVisitedRoute';
import type { NamedLocation } from './composables/useLastVisitedRoute';

// The tests call a redirect function with only the parts of the target
// location it reads (params, optionally query), not a full RouteLocation.
type RedirectFn = (to: { params: Record<string, string> }) => unknown;

// The redirect of the last matched record, which must be a function: the
// redirects under test all compute their target from the matched params.
function lastRedirect(matched: RouteLocationMatched[]): RedirectFn {
  const redirect = matched[matched.length - 1]?.redirect;
  if (typeof redirect !== 'function') {
    throw new Error(`expected a redirect function, got ${String(redirect)}`);
  }
  // A partial `to` is enough for these redirects; see RedirectFn.
  return redirect as unknown as RedirectFn;
}

// trajectTabTarget returns either a stored path verbatim or a named location;
// the tests that inspect name/params need the latter.
function expectNamed(t: string | NamedLocation): NamedLocation {
  if (typeof t === 'string') throw new Error(`expected a named location, got the path ${t}`);
  return t;
}

// A valid traject ref is `{slug}-{8hex}` (see the route regex). A plain
// law `$id` uses underscores and must NOT match the traject route.
const REF = 'mijn-traject-1a2b3c4d';

describe('route disambiguation (traject vs no-traject)', () => {
  it('routes a bare {slug}-{8hex} traject URL to traject-home', () => {
    const r = router.resolve(`/trajecten/${REF}`);
    expect(r.name).toBe('traject-home');
    expect(r.params.trajectRef).toBe(REF);
    expect(r.params.lawId).toBeUndefined();
  });

  it('routes a traject corpus URL to library-traject', () => {
    const r = router.resolve(`/trajecten/${REF}/corpus/zorgtoeslagwet/3`);
    expect(r.name).toBe('library-traject');
    expect(r.params.trajectRef).toBe(REF);
    expect(r.params.lawId).toBe('zorgtoeslagwet');
    expect(r.params.articleNumber).toBe('3');
  });

  it('routes a traject werkdocumenten URL to werkdocumenten-traject', () => {
    const r = router.resolve(`/trajecten/${REF}/werkdocumenten/map/besluit.md`);
    expect(r.name).toBe('werkdocumenten-traject');
    expect(r.params.trajectRef).toBe(REF);
    expect(r.params.docPath).toBe('map/besluit.md');
  });

  it('redirects the old standalone werkdocumenten URL into Home', () => {
    const matched = router.resolve(`/werkdocumenten/${REF}/besluit.md`).matched;
    const redirect = lastRedirect(matched);
    expect(redirect).toBeTruthy();
    expect(redirect({ params: { trajectRef: REF, docPath: 'besluit.md' } })).toMatchObject({
      name: 'werkdocumenten-traject',
      params: { trajectRef: REF, docPath: 'besluit.md' },
    });
  });

  it('routes a traject taken URL to taken-traject', () => {
    const r = router.resolve(`/trajecten/${REF}/taken`);
    expect(r.name).toBe('taken-traject');
    expect(r.params.trajectRef).toBe(REF);
    // Geen categorie = de "Kies een categorie"-staat in main.
    expect(r.params.categorie).toBeUndefined();
  });

  it('routes a taken categorie URL to taken-traject', () => {
    const r = router.resolve(`/trajecten/${REF}/taken/werkdocumenten`);
    expect(r.name).toBe('taken-traject');
    expect(r.params.categorie).toBe('werkdocumenten');
  });

  // A law CONTEXT filters the task list; it does not open the law. LibraryView
  // treats `params.lawId` as "a law is open here" - it selects that law in the
  // sidebar and fires loadLaw() for it - so the taken route must never produce
  // that param, however law-ish its URL looks.
  it('names a taken law context contextLawId, never lawId', () => {
    const r = router.resolve(`/trajecten/${REF}/taken/wet/wet_op_de_zorgtoeslag`);
    expect(r.name).toBe('taken-traject');
    expect(r.params.categorie).toBe('wet');
    expect(r.params.contextLawId).toBe('wet_op_de_zorgtoeslag');
    expect(r.params.lawId).toBeUndefined();
  });

  // De structuurcontrole deelt het `/trajecten/{ref}/...`-pad met de andere
  // AppShell-kinderen. Deze test pint vast dat hij daar niet door een van die
  // kinderen wordt opgeslokt - `instellingen/:tab?` ligt er vlak naast.
  it('routes a traject structuur-controle URL to traject-structuur-controle', () => {
    const r = router.resolve(`/trajecten/${REF}/structuur-controle`);
    expect(r.name).toBe('traject-structuur-controle');
    expect(r.params.trajectRef).toBe(REF);
    expect(r.meta.requiresAuth).toBe(true);
    expect(r.meta.title).toBe('Traject-structuur controleren');
  });

  it('routes a plain law-id corpus URL to corpus-juris (no traject)', () => {
    const r = router.resolve('/corpus-juris/wet_op_de_zorgtoeslag');
    expect(r.name).toBe('corpus-juris');
    expect(r.params.lawId).toBe('wet_op_de_zorgtoeslag');
    expect(r.params.trajectRef).toBeUndefined();
  });

  it('serves Home at the clean root and redirects the old public library URLs', () => {
    expect(router.resolve('/').name).toBe('home');
    const matched = router.resolve('/library/wet_op_de_zorgtoeslag').matched;
    const redirect = lastRedirect(matched);
    expect(redirect).toBeTruthy();
    expect(redirect({ params: { lawId: 'wet_op_de_zorgtoeslag' } })).toMatchObject({
      name: 'corpus-juris',
      params: { lawId: 'wet_op_de_zorgtoeslag' },
    });
  });

  it('redirects the old traject library URLs onto /trajecten/{ref}', () => {
    // Bare old traject library -> traject-home.
    let matched = router.resolve(`/library/${REF}`).matched;
    let redirect = lastRedirect(matched);
    expect(redirect).toBeTruthy();
    expect(redirect({ params: { trajectRef: REF } })).toMatchObject({
      name: 'traject-home',
      params: { trajectRef: REF },
    });
    // With a law -> library-traject (the /corpus/{law} segment).
    matched = router.resolve(`/library/${REF}/zorgtoeslagwet/3`).matched;
    redirect = lastRedirect(matched);
    expect(redirect).toBeTruthy();
    expect(
      redirect({ params: { trajectRef: REF, lawId: 'zorgtoeslagwet', articleNumber: '3' } }),
    ).toMatchObject({
      name: 'library-traject',
      params: { trajectRef: REF, lawId: 'zorgtoeslagwet' },
    });
  });

  it('mirrors the same split for the editor routes', () => {
    expect(router.resolve(`/trajecten/${REF}/editor/foo`).name).toBe('editor-traject');
    // The chooser lives at its own section-neutral URL (/trajecten); the bare
    // /editor redirects onto it with sectie=editor, and a no-traject law URL
    // does the same with the law carried as query.
    expect(router.resolve('/trajecten').name).toBe('trajecten');
    expect(router.resolve('/editor/nieuw-traject').name).toBe('editor-nieuw-traject');
    // A no-traject law URL hits a redirect route (router.resolve returns the
    // redirect record without following it). Assert it redirects onto the
    // chooser, carrying the section + law as query.
    const matched = router.resolve('/editor/wet_op_de_zorgtoeslag').matched;
    const redirect = lastRedirect(matched);
    expect(redirect).toBeTruthy();
    expect(redirect({ params: { lawId: 'wet_op_de_zorgtoeslag' } })).toMatchObject({
      name: 'trajecten',
      query: { sectie: 'editor', law: 'wet_op_de_zorgtoeslag' },
    });
  });

  it('redirects the old traject editor URLs onto /trajecten/{ref}/editor', () => {
    const matched = router.resolve(`/editor/${REF}/foo`).matched;
    const redirect = lastRedirect(matched);
    expect(redirect).toBeTruthy();
    expect(redirect({ params: { trajectRef: REF, lawId: 'foo' } })).toMatchObject({
      name: 'editor-traject',
      params: { trajectRef: REF, lawId: 'foo' },
    });
  });
});

describe('sectionTarget - traject preserved across tab switches', () => {
  it('stamps the active traject when entering a section without one', () => {
    const t = sectionTarget(router, '/corpus-juris/wet_op_de_zorgtoeslag', REF);
    expect(t.name).toBe('library-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.lawId).toBe('wet_op_de_zorgtoeslag');
  });

  it('drops a foreign lawId on a cross-traject editor path, landing on the editor root', () => {
    // The stored editor path names a DIFFERENT traject: its lawId belongs to
    // that traject and may not exist in the active one. Stamping it across gave
    // a "niet beschikbaar in dit traject" dead end, so we now land on the active
    // traject's editor root and let the restore-on-entry flow open the right
    // article (same rule as trajectSwitchTarget).
    const t = sectionTarget(router, '/trajecten/old-deadbeef/editor/foo', REF);
    expect(t.name).toBe('editor-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.lawId).toBeUndefined();
  });

  it('strips the traject when none is active (Geen traject)', () => {
    const t = sectionTarget(router, `/trajecten/${REF}/corpus/foo`, null);
    expect(t.name).toBe('corpus-juris');
    expect(t.params.trajectRef).toBeUndefined();
    expect(t.params.lawId).toBe('foo');
  });

  it('preserves the werkdocumenten sub-mode across a tab switch', () => {
    const t = sectionTarget(router, `/trajecten/${REF}/werkdocumenten/besluit.md`, REF);
    expect(t.name).toBe('werkdocumenten-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.docPath).toBe('besluit.md');
  });

  it('re-stamps the active traject onto a stored werkdocumenten path', () => {
    const t = sectionTarget(router, '/trajecten/old-deadbeef/werkdocumenten/x.md', REF);
    expect(t.name).toBe('werkdocumenten-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.docPath).toBe('x.md');
  });

  it('drops the werkdocumenten sub-mode when no traject is active', () => {
    const t = sectionTarget(router, `/trajecten/${REF}/werkdocumenten/x.md`, null);
    expect(t.name).toBe('home');
  });

  it('preserves the taken sub-mode across a tab switch, re-stamping the active traject', () => {
    const t = sectionTarget(router, '/trajecten/old-deadbeef/taken', REF);
    expect(t.name).toBe('taken-traject');
    expect(t.params.trajectRef).toBe(REF);
  });

  it('drops the taken sub-mode when no traject is active', () => {
    const t = sectionTarget(router, `/trajecten/${REF}/taken`, null);
    expect(t.name).toBe('home');
  });

  it('re-stamps the active traject onto a stored instellingen path, keeping the tab', () => {
    const t = sectionTarget(router, '/trajecten/old-deadbeef/instellingen/leden', REF);
    expect(t.name).toBe('instellingen-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.tab).toBe('leden');
  });

  it('preserves the instellingen sub-mode without a tab', () => {
    const t = sectionTarget(router, `/trajecten/${REF}/instellingen`, REF);
    expect(t.name).toBe('instellingen-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.tab).toBeUndefined();
  });

  it('drops the instellingen sub-mode when no traject is active', () => {
    const t = sectionTarget(router, `/trajecten/${REF}/instellingen/details`, null);
    expect(t.name).toBe('home');
  });

  it('sends the Editor tab to the chooser when no traject is active', () => {
    // The editor requires a traject: with none active, the stored editor
    // path collapses to the chooser and the law travels along as query.
    const t = sectionTarget(router, `/trajecten/${REF}/editor/foo/3`, null);
    expect(t.name).toBe('editor');
    expect(t.params.trajectRef).toBeUndefined();
    expect(t.query).toEqual({ law: 'foo', article: '3' });
  });

  it('upgrades a stored chooser path to the active traject editor', () => {
    const t = sectionTarget(router, '/editor?law=foo', REF);
    expect(t.name).toBe('editor-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.lawId).toBe('foo');
  });

  it('falls back to the section root for an unresolvable stored path', () => {
    // A corrupted/stale sessionStorage value must not crash router.push.
    expect(sectionTarget(router, '/totally/unknown', REF).name).toBe('traject-home');
    expect(sectionTarget(router, '/totally/unknown', null).name).toBe('home');
    // Section is derived from the path prefix so the right tab is kept.
    expect(sectionTarget(router, '/editor-bogus/x', null).name).toBe('editor');
  });
});

describe('top-level tabs - which section a route belongs to', () => {
  it('puts the public landing and a public law under Corpus juris', () => {
    expect(isCorpusSection('home')).toBe(true);
    expect(isCorpusSection('corpus-juris')).toBe(true);
    expect(isTrajectSection('corpus-juris')).toBe(false);
  });

  it('puts every traject page under Traject, not Corpus juris', () => {
    for (const name of ['traject-home', 'library-traject', 'werkdocumenten-traject',
      'instellingen-traject', 'taken-traject', 'traject-structuur-controle']) {
      expect(isTrajectSection(name)).toBe(true);
      expect(isCorpusSection(name)).toBe(false);
    }
  });

  it('leaves the editor in neither', () => {
    expect(isCorpusSection('editor-traject')).toBe(false);
    expect(isTrajectSection('editor-traject')).toBe(false);
  });
});

describe('trajectTabTarget - Traject tab', () => {
  it('opens the traject chooser when no traject is known', () => {
    const t = expectNamed(trajectTabTarget(router, null, null));
    expect(t.name).toBe('trajecten');
    expect(t.query).toEqual({ sectie: 'library' });
  });

  it('opens the traject home when the traject section has no stored page yet', () => {
    const t = expectNamed(trajectTabTarget(router, null, REF));
    expect(t.name).toBe('traject-home');
    expect(t.params.trajectRef).toBe(REF);
  });

  it('restores the stored path verbatim when it belongs to the traject (hash intact)', () => {
    const stored = `/trajecten/${REF}/corpus/foo/3#yaml`;
    expect(trajectTabTarget(router, stored, REF)).toBe(stored);
  });

  it('re-stamps the traject over a stale stored traject', () => {
    const t = expectNamed(trajectTabTarget(router, '/trajecten/old-deadbeef/corpus/foo', REF));
    expect(t.name).toBe('library-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.lawId).toBe('foo');
  });

  it('keeps the instellingen sub-mode when re-stamping onto another traject', () => {
    const t = expectNamed(trajectTabTarget(router, '/trajecten/old-deadbeef/instellingen/details', REF));
    expect(t.name).toBe('instellingen-traject');
    expect(t.params.trajectRef).toBe(REF);
    expect(t.params.tab).toBe('details');
  });
});

describe('repeated query values take the first value', () => {
  // `?law=a&law=b` makes vue-router give the query value as an array. Passed
  // into a route param as is, resolving the location throws ("Provided param
  // "lawId" is an array but it is not repeatable").
  it('upgrades a stored chooser path with a repeated law to a resolvable editor location', () => {
    const t = sectionTarget(router, '/editor?law=foo&law=bar&article=3&article=4', REF);
    expect(t.params.lawId).toBe('foo');
    expect(t.params.articleNumber).toBe('3');
    expect(() => router.resolve(t)).not.toThrow();
    expect(router.resolve(t).path).toBe(`/trajecten/${REF}/editor/foo/3`);
  });

  it('redirects /editor.html with a repeated or valueless law to a single law', () => {
    const matched = router.resolve('/editor.html').matched;
    const redirect = lastRedirect(matched) as unknown as (to: {
      query: Record<string, string | null | (string | null)[]>;
    }) => { query: Record<string, unknown> };
    expect(redirect({ query: { law: ['foo', 'bar'], article: ['3', '4'] } }).query)
      .toEqual({ sectie: 'editor', law: 'foo', article: '3' });
    expect(redirect({ query: { law: null } }).query.law).toBeUndefined();
  });
});
