import { ref, computed } from 'vue';
import type {
  LocationQueryRaw,
  RouteParamsRawGeneric,
  RouteRecordNameGeneric,
  Router,
} from 'vue-router';
import { singleQuery } from '../lib/routeParams';

/**
 * The stored map: section key ('home', 'editor', or a route name) to the
 * last visited full path. Its keys are the sessionStorage format.
 */
type LastVisited = Record<string, string | undefined>;

/** A named route location as this module builds it, for `router.push`. */
export interface NamedLocation {
  name: string;
  params: RouteParamsRawGeneric;
  query?: LocationQueryRaw;
}

type ParamValue = RouteParamsRawGeneric[string];

export interface HomeTargetOptions {
  trajectRef?: ParamValue;
  lawId?: ParamValue;
  articleNumber?: ParamValue;
}

/** The part of the router this module uses. */
type RouteResolver = Pick<Router, 'resolve'>;

// Track the last visited fullPath per route name so the top-level
// Corpus juris/Traject/Editor tab switch returns the user to where they were
// instead of the section root.
//
// Backed by sessionStorage so a refresh keeps the state for the same
// tab session, but doesn't bleed across browser tabs/windows.

const STORAGE_KEY = 'regelrecht:last-visited';

function load(): LastVisited {
  try {
    return JSON.parse(sessionStorage.getItem(STORAGE_KEY) ?? '{}');
  } catch {
    return {};
  }
}

const _lastVisited = ref<LastVisited>(load());

function save() {
  try {
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify(_lastVisited.value));
  } catch {
    /* storage may be disabled / quota exceeded - fall back to memory only */
  }
}

// The two halves of what used to be one "Home" section. Corpus juris is the
// public library (the landing and a law in it); the traject section is a
// traject's own home with its corpus, werkdocumenten, instellingen and taken.
// Each has its own top-level tab, so each remembers its own last page.
export const CORPUS_ROUTE_NAMES = ['home', 'corpus-juris'];
export const TRAJECT_ROUTE_NAMES = [
  'traject-home',
  'library-traject',
  'werkdocumenten-traject',
  'instellingen-traject',
  'taken-traject',
  'traject-structuur-controle',
];
// Every route LibraryView renders, in either half. Kept for the checks that
// only ask "library or editor?" (a traject switch, login-to-chooser).
export const HOME_ROUTE_NAMES = [...CORPUS_ROUTE_NAMES, ...TRAJECT_ROUTE_NAMES];

type RouteName = RouteRecordNameGeneric | null | undefined;

export function isCorpusSection(routeName: RouteName): boolean {
  return (CORPUS_ROUTE_NAMES as readonly unknown[]).includes(routeName);
}
export function isTrajectSection(routeName: RouteName): boolean {
  return (TRAJECT_ROUTE_NAMES as readonly unknown[]).includes(routeName);
}
export function isHomeSection(routeName: RouteName): boolean {
  return isCorpusSection(routeName) || isTrajectSection(routeName);
}

// Build a Home-section route location. With a traject: the bare traject home, or
// its corpus when a law is open. Without one: the public bare landing, or a
// public law. Centralises the with/without-law + with/without-traject split used
// by every Home navigation (tab switch, traject switch, chooser, in-view nav).
export function homeTarget({ trajectRef, lawId, articleNumber }: HomeTargetOptions = {}): NamedLocation {
  if (trajectRef) {
    return lawId
      ? { name: 'library-traject', params: { trajectRef, lawId, articleNumber } }
      : { name: 'traject-home', params: { trajectRef } };
  }
  return lawId
    ? { name: 'corpus-juris', params: { lawId, articleNumber } }
    : { name: 'home', params: {} };
}

// Where a traject switch should land: the ROOT of the section you're in
// (Home or editor), in the newly picked traject. A switch deliberately does
// NOT carry the law/article you were viewing - the new traject's corpus
// differs, so the old lawId points at a document it doesn't have. Carrying it
// across made the editor try to open that foreign document (and leak the
// previous traject's tab). Landing on the bare root instead lets the editor
// show the new traject's own saved tabs in the bar without auto-opening one.
export function trajectSwitchTarget(
  routeName: RouteRecordNameGeneric | null | undefined,
  trajectRef: ParamValue,
): NamedLocation {
  return isHomeSection(routeName)
    ? homeTarget({ trajectRef })
    : { name: 'editor-traject', params: { trajectRef } };
}

function storageKeyFor(routeName: RouteRecordNameGeneric): string {
  if (routeName === 'editor-traject') return 'editor';
  if (isCorpusSection(routeName)) return 'corpus';
  if (isTrajectSection(routeName)) return 'traject';
  // Route names in this app are strings, never symbols.
  return routeName as string;
}

export function recordLastVisited(
  routeName: RouteRecordNameGeneric | null | undefined,
  fullPath: string,
  trajectRef?: string | null,
): void {
  if (!routeName) return;
  // Mutate in place rather than re-spreading. The ref wraps a reactive
  // object so a property assignment still notifies the computeds below.
  const key = storageKeyFor(routeName);
  _lastVisited.value[key] = fullPath;
  // 'home' is the last page of either library half, for the places that
  // return "to where you were browsing" (the traject chooser's back button).
  if (key === 'corpus' || key === 'traject') _lastVisited.value.home = fullPath;
  // The last traject you were in, from any section. The Traject and Editor
  // tabs fall back to it while you are in Corpus juris, which has no traject.
  if (trajectRef) _lastVisited.value.trajectRef = trajectRef;
  save();
}

// The last page of either library half (Corpus juris or a traject's home).
export const lastHomePath = computed(() => _lastVisited.value.home ?? '/');

// The last Corpus juris page; the Corpus juris tab restores it verbatim.
export const lastCorpusPath = computed(() => _lastVisited.value.corpus ?? '/');

// The last traject-section page and the last traject seen anywhere, or null.
export const lastTrajectPath = computed(() => _lastVisited.value.traject ?? null);
export const lastTrajectRef = computed(() => _lastVisited.value.trajectRef ?? null);

// `/editor` (no traject) is the read-only editor. The first visit
// lands there; subsequent visits restore the most-recently-seen
// editor URL - the traject chooser OR a traject-scoped editor,
// whichever the user was on last.
export const lastEditorPath = computed(
  () => _lastVisited.value.editor ?? '/editor',
);

// The page to return to when leaving the harvester — the route you were on when
// you entered it (Home or Editor). Backed by sessionStorage so a refresh inside
// the harvester still remembers where to go back; falls back to the root.
const HARVESTER_RETURN_KEY = 'regelrecht:harvester-return';
export function rememberHarvesterOrigin(fullPath: string): void {
  try {
    sessionStorage.setItem(HARVESTER_RETURN_KEY, fullPath);
  } catch {
    /* storage disabled / quota — the fallback path handles it */
  }
}
export function harvesterReturnPath(): string {
  try {
    return sessionStorage.getItem(HARVESTER_RETURN_KEY) || '/';
  } catch {
    return '/';
  }
}

// Build a router target for the *other* top-level section (Bibliotheek /
// Editor) from its last-visited path, but re-stamp the trajectRef to the
// CURRENTLY active traject. The stored path can carry a stale trajectRef
// (or none) from an earlier visit; the active scope must win, otherwise a
// tab-switch would either drop the traject (the bug this fixes) or
// teleport the user into whatever traject they last had open in that
// section. Returns a `{ name, params }` location for `router.push`.
export function sectionTarget(
  router: RouteResolver,
  storedPath: string,
  activeRef: string | null | undefined,
): NamedLocation {
  const loc = router.resolve(storedPath);
  const params: RouteParamsRawGeneric = { ...loc.params };
  const name = loc.name;

  // Editor section = /editor* or /trajecten/{ref}/editor* (or an unresolved
  // path under /editor); everything else is the Home section (home /
  // corpus-juris / traject-home / library-traject).
  const isEditor = name === 'editor' || name === 'editor-traject'
    || (name == null && storedPath.startsWith('/editor'));

  if (isEditor) {
    if (activeRef) {
      const storedRef = loc.params.trajectRef || null;
      // Cross-traject: the stored path names a DIFFERENT traject, so its lawId
      // belongs to that traject and may not exist here. Do NOT stamp it onto
      // the active traject (that produced the "niet beschikbaar in dit traject"
      // dead end). Land on the active traject's editor root and let the
      // restore-on-entry flow open the right article - the same rule as
      // trajectSwitchTarget. (A traject-less chooser/global editor path carries
      // an unscoped law and is handled below.)
      if (storedRef && storedRef !== activeRef) {
        return { name: 'editor-traject', params: { trajectRef: activeRef } };
      }
      params.trajectRef = activeRef;
      // A stored chooser path carries the intended law as query.
      const lawId = singleQuery(loc.query?.law);
      const articleNumber = singleQuery(loc.query?.article);
      if (lawId) params.lawId = lawId;
      if (articleNumber) params.articleNumber = articleNumber;
      return { name: 'editor-traject', params };
    }
    // The editor requires a traject: without one the Editor tab goes to the
    // chooser, carrying any law as query so it opens after a traject is picked.
    const query: LocationQueryRaw = { ...(loc.query ?? {}) };
    if (params.lawId) query.law = params.lawId;
    if (params.articleNumber) query.article = params.articleNumber;
    return { name: 'editor', params: {}, query };
  }

  // Werkdocumenten sub-mode of Home: preserve it (re-stamp the active traject,
  // keep the open document) so a Traject↔Editor tab switch returns to the open
  // werkdocument. Without a traject the werkdoc route doesn't apply, so it falls
  // through to the corpus shapes below.
  const isWerkdoc = name === 'werkdocumenten-traject'
    || (name == null && storedPath.includes('/werkdocumenten'));
  if (isWerkdoc && activeRef) {
    const target: NamedLocation = { name: 'werkdocumenten-traject', params: { trajectRef: activeRef } };
    if (params.docPath) target.params.docPath = params.docPath;
    return target;
  }

  // Taken sub-mode of Home: same preservation as werkdocumenten (re-stamp the
  // active traject) so a Traject↔Editor tab switch returns to the open taken-panel.
  const isTaken = name === 'taken-traject'
    || (name == null && /\/taken($|[/?#])/.test(storedPath));
  if (isTaken && activeRef) {
    return { name: 'taken-traject', params: { trajectRef: activeRef } };
  }

  // Instellingen sub-mode of Home: same preservation as werkdocumenten
  // (re-stamp the active traject, keep the open tab). Without this branch the
  // generic fallback below would silently collapse a stored instellingen path
  // to the traject home on a scope mismatch.
  const isInstellingen = name === 'instellingen-traject'
    || (name == null && /\/instellingen($|[/?#])/.test(storedPath));
  if (isInstellingen && activeRef) {
    const target: NamedLocation = { name: 'instellingen-traject', params: { trajectRef: activeRef } };
    if (params.tab) target.params.tab = params.tab;
    return target;
  }

  // Home section (corpus): the active traject scope wins over whatever the
  // stored path carried (re-stamped, or dropped when browsing without a traject).
  return homeTarget({
    trajectRef: activeRef || undefined,
    lawId: params.lawId,
    articleNumber: params.articleNumber,
  });
}

// Shared same-traject shortcut for the top-level tab targets: when the stored
// path already carries the active scope, return it VERBATIM so query/hash (a
// werkdocument's `?task=`, the #yaml detail tab, the editor's exact article)
// survive exactly; otherwise re-stamp/rebuild via sectionTarget. Both the Home
// and Editor tab reuse this instead of each duplicating the check.
function tabTargetFor(
  router: RouteResolver,
  storedPath: string,
  activeRef: string | null | undefined,
): string | NamedLocation {
  const storedRef = router.resolve(storedPath).params.trajectRef || null;
  if (storedRef === (activeRef || null)) return storedPath;
  return sectionTarget(router, storedPath, activeRef);
}

// Target for the Traject tab. `ref` is the traject to show: the one in the URL,
// or (from Corpus juris, which has none) the last traject you were in. Without
// one there is nothing to return to, so the tab opens the traject chooser.
// With one: the last traject page verbatim when it belongs to that traject,
// re-stamped onto it otherwise, or the traject home when there is none yet.
export function trajectTabTarget(
  router: RouteResolver,
  storedPath: string | null,
  ref: string | null | undefined,
): string | NamedLocation {
  if (!ref) return { name: 'trajecten', params: {}, query: { sectie: 'library' } };
  if (!storedPath) return { name: 'traject-home', params: { trajectRef: ref } };
  return tabTargetFor(router, storedPath, ref);
}

// Target for the Editor tab: same same-traject-verbatim / cross-traject-rebuild
// rule as the Traject tab. On a cross-traject stored path sectionTarget now lands
// on the active traject's editor ROOT (dropping the foreign lawId), and the
// editor's restore-on-entry flow opens the right article.
export function editorTabTarget(
  router: RouteResolver,
  storedPath: string,
  activeRef: string | null | undefined,
): string | NamedLocation {
  return tabTargetFor(router, storedPath, activeRef);
}
