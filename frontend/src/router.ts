import { createRouter, createWebHistory } from 'vue-router';
import AppShell from './AppShell.vue';
import LibraryView from './LibraryView.vue';
import { ensureAuthReady, useAuth } from './composables/useAuth';
import { recordLastVisited } from './composables/useLastVisitedRoute';
import { singleQuery } from './lib/routeParams';

// The meta fields the routes below set and the guards read. `meta` is merged
// across matched records, so a child sees its parent's fields.
declare module 'vue-router' {
  interface RouteMeta {
    title?: string;
    requiresAuth?: boolean;
    requiresRole?: readonly string[];
  }
}

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      // Persistent shell: holds the shared chrome (toolbars, tab-bar,
      // settings menu, traject-switcher) and a nested <router-view> for the
      // section bodies. Editor and library are children of this one record,
      // so switching between them reuses the shell instance - only the nested
      // router-view swaps - and the chrome never rebuilds on a tab switch.
      path: '/',
      component: AppShell,
      children: [
        // Home (public): the bare landing shows Corpus juris with a clean URL;
        // picking a law drills into /corpus-juris/{lawId}. Both render the same
        // LibraryView, so selecting/clearing a law reuses the component instead
        // of remounting.
        {
          path: '',
          name: 'home',
          component: LibraryView,
          meta: { title: 'Home' },
        },
        {
          path: 'corpus-juris/:lawId/:articleNumber?',
          name: 'corpus-juris',
          component: LibraryView,
          meta: { title: 'Home' },
        },
        {
          // Traject-scoped Home landing: the same LibraryView, read through
          // `/api/trajects/{trajectRef}/corpus/...` so the active traject
          // survives a Home↔Editor tab switch. The active traject lives in the
          // URL (per-tab state), never a server session.
          //
          // The `:trajectRef` regex pins the param to `{slug}-{8hex}` so a plain
          // law-id slug like `wet_op_de_zorgtoeslag` can never be mistaken for a
          // traject ref. Bare `/trajecten/{ref}` is the traject landing.
          path: 'trajecten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})',
          name: 'traject-home',
          component: LibraryView,
          meta: { title: 'Home', requiresAuth: true },
        },
        {
          // A law opened within a traject: `/trajecten/{ref}/corpus/{lawId}`.
          // Kept named 'library-traject' so existing navigations keep working.
          path: 'trajecten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/corpus/:lawId/:articleNumber?',
          name: 'library-traject',
          component: LibraryView,
          meta: { title: 'Home', requiresAuth: true },
        },
        {
          // Werkdocumenten within a traject: `/trajecten/{ref}/werkdocumenten/{docPath?}`.
          // Rendered by the same LibraryView (Home), which shows the document list
          // in its secondary sidebar and the editor in main. `docPath` uses `(.*)`
          // so folder paths (a/b.md) match, mirroring the old standalone route.
          path: 'trajecten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/werkdocumenten/:docPath(.*)?',
          name: 'werkdocumenten-traject',
          component: LibraryView,
          meta: { title: 'Werkdocumenten', requiresAuth: true },
        },
        {
          // Traject settings within a traject: details + members, rendered by the
          // same LibraryView (Instellingen entry -> secondary-sidebar tabs ->
          // content in main). `:tab` is pinned to the two panes.
          path: 'trajecten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/instellingen/:tab(details|leden)?',
          name: 'instellingen-traject',
          component: LibraryView,
          meta: { title: 'Instellingen', requiresAuth: true },
        },
        {
          // Structuurcontrole van een traject: `/trajecten/{ref}/structuur-controle`.
          // Zelfde LibraryView als Instellingen: deze route toont de
          // Algemeen-pane met het rapport als sheet erover. Zo houdt het rapport
          // een deelbaar adres zonder een eigen pagina te zijn, en is de URL
          // meteen de open-stand van die sheet.
          //
          // Geen `traject-` in het pad: `/trajecten/` staat er al voor.
          path: 'trajecten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/structuur-controle',
          name: 'traject-structuur-controle',
          component: LibraryView,
          meta: { title: 'Traject-structuur controleren', requiresAuth: true },
        },
        {
          // The user's taken within a traject: `/trajecten/{ref}/taken`.
          // Rendered by the same LibraryView, shaped like Instellingen: the
          // secondary sidebar lists the categories, the chosen one opens its
          // task list in main. A task itself still never opens in main -
          // "Beoordelen" navigates to the editor or the addressed werkdocument.
          //
          // `:categorie` is pinned to the panel entries. `:contextLawId` names
          // the law behind a `wet` context (`/taken/wet/kieswet`) and is
          // deliberately NOT pinned: the law contexts are derived from whatever
          // laws the open tasks mention, so the set is data-driven, not a fixed
          // enum. An unknown id simply yields an empty list.
          //
          // It is emphatically NOT called `lawId`: LibraryView treats that param
          // as "a law is open in this view" and both selects it in the sidebar
          // and fires loadLaw() for it (see its route.params.lawId watcher and
          // initial load). A task context filters a list - it opens no law - so
          // reusing the name would highlight and fetch a law nobody asked for.
          path: 'trajecten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/taken/:categorie(alle|prioriteit|wachten|werkdocumenten|wet)?/:contextLawId?',
          name: 'taken-traject',
          component: LibraryView,
          meta: { title: 'Taken', requiresAuth: true },
        },
        // Back-compat: the old public library URLs redirect to the new paths.
        // Declared AFTER library-traject so a {slug}-{8hex} ref still matches
        // the traject route rather than these plain-law redirects.
        { path: 'corpus-juris', redirect: '/' },
        { path: 'library', redirect: '/' },
        {
          // Old traject bibliotheek: /library/{ref}[/{law}[/{art}]] ->
          // /trajecten/{ref}[/corpus/{law}[/{art}]]. Regex-pinned so it matches a
          // {slug}-{8hex} ref; declared BEFORE the plain-law redirect below.
          path: 'library/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/:articleNumber?',
          redirect: (to) => (to.params.lawId
            ? {
                name: 'library-traject',
                params: {
                  trajectRef: to.params.trajectRef,
                  lawId: to.params.lawId,
                  articleNumber: to.params.articleNumber || undefined,
                },
              }
            : { name: 'traject-home', params: { trajectRef: to.params.trajectRef } }),
        },
        {
          path: 'library/:lawId/:articleNumber?',
          redirect: (to) => ({
            name: 'corpus-juris',
            params: { lawId: to.params.lawId, articleNumber: to.params.articleNumber || undefined },
          }),
        },
        {
          // Traject-scoped editor: full read + write. Per-tab active traject
          // lives in the URL. API hangs under `/api/trajects/{trajectRef}/...`.
          //
          // **Invariant**: law `$id` slugs must not match the `{slug}-{8hex}`
          // regex (they use underscores, e.g. `wet_op_de_zorgtoeslag`), so a
          // plain law id can never be mistaken for a traject ref.
          path: 'trajecten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/editor/:lawId?/:articleNumber?',
          name: 'editor-traject',
          component: () => import('./EditorView.vue'),
          meta: { title: 'Editor', requiresAuth: true },
        },
        {
          // De kale /editor vereist nog steeds een traject: door naar de
          // chooser met sectie=editor (meegegeven query blijft behouden). De
          // naam 'editor' blijft bestaan zodat alle bestaande
          // `{ name: 'editor' }`-navigaties (Editor-tab, sectionTarget,
          // redirects) hier doorheen naar de chooser lopen.
          path: 'editor',
          name: 'editor',
          redirect: (to) => ({ name: 'trajecten', query: { sectie: 'editor', ...to.query } }),
        },
        {
          // Old traject editor: /editor/{ref}[/{law}[/{art}]] ->
          // /trajecten/{ref}/editor[/{law}[/{art}]]. Regex-pinned, declared
          // BEFORE the plain-law editor redirect below so a {slug}-{8hex} ref
          // matches the traject path, not a law path.
          path: 'editor/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/:articleNumber?',
          redirect: (to) => ({
            name: 'editor-traject',
            params: {
              trajectRef: to.params.trajectRef,
              lawId: to.params.lawId || undefined,
              articleNumber: to.params.articleNumber || undefined,
            },
          }),
        },
        {
          // Editor-links zonder traject (de vroegere read-only editor): er is
          // geen editor zonder traject meer. Door naar de keuzepagina, met de
          // wet als query zodat die na de keuze opent.
          //
          // Staat bewust NA `editor-traject`: een traject-ref-URL
          // ({slug}-{8hex}) moet de traject-route matchen, niet deze redirect.
          // De declaratievolgorde is hier de scheidsrechter.
          path: 'editor/:lawId/:articleNumber?',
          redirect: (to) => ({
            name: 'trajecten',
            query: {
              sectie: 'editor',
              law: to.params.lawId,
              article: to.params.articleNumber || undefined,
            },
          }),
        },
      ],
    },
    {
      // Trajectchooser - sectie-neutrale URL (library|editor via `sectie`,
      // default editor) zodat zowel de bibliotheek als de editor 'm gebruiken;
      // `law`/`article` dragen de beoogde bestemming mee. Top-level route (geen
      // AppShell-child), zoals werkdocumenten: geen app-chrome, de pagina draagt
      // z'n eigen top-title-bar met terugknop naar de bibliotheek.
      path: '/trajecten',
      name: 'trajecten',
      component: () => import('./TrajectChooserView.vue'),
      meta: { title: 'Trajecten', requiresAuth: true },
    },
    {
      // Nieuw traject aanmaken - eigen pagina met het gedeelde aanmaakformulier
      // (TrajectCreateForm). Ook top-level (geen app-chrome). Het statische pad
      // `/editor/nieuw-traject` scoort boven de dynamische `/editor/...`-routes
      // in de AppShell, dus een traject-ref of wet-id matcht deze nooit.
      path: '/editor/nieuw-traject',
      name: 'editor-nieuw-traject',
      component: () => import('./TrajectCreateView.vue'),
      meta: { title: 'Nieuw traject', requiresAuth: true },
    },
    {
      // Bare "session expired / logged out" page. Top-level (no app-chrome) and
      // PUBLIC (no requiresAuth) so the auth guards can land here without
      // looping. Both the router guard and the fetch 401-guard redirect here
      // with `?return_url=<the protected page>` when a session expires, instead
      // of the old silent bounce to /auth/login (which could dead-end blank).
      // The page's re-login button forwards return_url to /auth/login so the
      // user comes back where they were.
      path: '/uitgelogd',
      name: 'uitgelogd',
      component: () => import('./SessionExpiredView.vue'),
      meta: { title: 'Uitgelogd' },
    },
    {
      // Harvester-admin "Corpusinwinning" section - the merged harvester dashboard.
      // Top-level route (sibling of AppShell, not nested) so it carries its own
      // chrome (HarvesterView), with the two sub-screens as nested children -
      // mirroring the original standalone admin dashboard. Gated on any
      // harvester-* role via `meta.requiresRole` (checked in `beforeEach`);
      // write actions inside are enforced server-side by the harvester-admin
      // API. Child routes inherit this record's meta.
      path: '/harvesting',
      component: () => import('./harvester/HarvesterView.vue'),
      meta: {
        title: 'Harvester',
        requiresAuth: true,
        requiresRole: [
          'harvester-reader',
          'harvester-writer',
          'harvester-admin',
          'regelrecht-admin',
        ],
      },
      children: [
        { path: '', redirect: '/harvesting/overview' },
        {
          path: 'overview',
          name: 'overview',
          component: () => import('./harvester/views/OverviewView.vue'),
        },
        {
          path: 'law-entries',
          name: 'law-entries',
          component: () => import('./harvester/views/LawEntriesView.vue'),
        },
        {
          path: 'jobs',
          name: 'jobs',
          component: () => import('./harvester/views/JobsView.vue'),
        },
        {
          path: 'markings',
          name: 'markings',
          component: () => import('./harvester/views/MarkingsView.vue'),
        },
        {
          path: 'untranslatables',
          name: 'untranslatables',
          component: () => import('./harvester/views/UntranslatablesView.vue'),
        },
      ],
    },
    {
      // Account aanvragen - publieke uitlegpagina (geen requiresAuth),
      // bereikbaar vanaf de login-warning-popover. Top-level route met een
      // eigen top-title-bar, geen app-chrome (net als de trajecten-pagina's).
      path: '/account-aanvragen',
      name: 'account-aanvragen',
      component: () => import('./AccountRequestView.vue'),
      meta: { title: 'Account aanvragen' },
    },
    {
      // Back-compat: the old standalone /werkdocumenten/{ref}/{docPath} URLs now
      // live inside Home at /trajecten/{ref}/werkdocumenten/{docPath}.
      path: '/werkdocumenten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:docPath(.*)?',
      redirect: (to) => ({
        name: 'werkdocumenten-traject',
        params: { trajectRef: to.params.trajectRef, docPath: to.params.docPath || undefined },
      }),
    },
    {
      path: '/editor.html',
      redirect: (to) => ({
        name: 'trajecten',
        query: {
          sectie: 'editor',
          law: singleQuery(to.query.law) || undefined,
          article: singleQuery(to.query.article) || undefined,
        },
      }),
    },
  ],
});

// Gate any route marked `meta.requiresAuth` on the auth-status check. We
// block the client-side navigation until `/auth/status` has resolved, so the
// target component never mounts until we know the user may enter.
// Unauthenticated users are always sent to `/auth/login` and the navigation
// is cancelled - the previous route stays visible until the browser leaves,
// instead of flashing the protected UI. Deliberately NOT conditional on
// `oidcConfigured`: the editor must never open without login, including
// environments without OIDC (there `/auth/login` either serves the dev login
// or surfaces a backend error). A failed /auth/status check leaves
// `authenticated` false and thus fails closed.
router.beforeEach(async (to) => {
  if (!to.meta.requiresAuth) return true;
  await ensureAuthReady();
  const { authenticated, hasAnyRole } = useAuth();
  if (!authenticated.value) {
    // Send to the bare "je bent uitgelogd" page instead of silently bouncing
    // through /auth/login (which could dead-end on a blank page). It carries
    // the intended destination so its re-login button returns the user here.
    return { name: 'uitgelogd', query: { return_url: to.fullPath } };
  }
  // Role-gated routes (e.g. the harvester-admin Corpusinwinning section): an
  // authenticated user lacking the required role is redirected to the
  // library rather than bounced through login (which would loop, since
  // logging in again yields the same role set). `requiresRole` is a list of
  // acceptable roles; holding any one grants access. `meta` is merged across
  // matched records, so a child inherits its parent's `requiresRole`.
  if (to.meta.requiresRole && !hasAnyRole(to.meta.requiresRole)) {
    return { path: '/' };
  }
  return true;
});

// Track the last fullPath per route name so the Bibliotheek/Editor tab
// switch can restore the user's prior position in each section.
router.afterEach((to) => {
  recordLastVisited(to.name, to.fullPath);
});

// document.title is owned by the route components (they reflect law + article
// state) via watchEffect - see the note that used to live here on main.

export default router;
