---
title: "Editor"
description: "The Vue 3 law editor: the corpus browser, trajects with their werkdocumenten and tasks, the split-pane editor, and the harvester dashboard."
---

The editor is a Vue 3 single-page app in `frontend/`. It browses the corpus, holds the traject workspace (laws, werkdocumenten, tasks, members), edits laws in a split-pane editor with scenarios running on the engine as WASM, and hosts the harvester dashboard.

## Overview

- **Framework**: Vue 3 (Composition API), Vue Router
- **Language**: TypeScript, on the setup [Frontend Code](/guide/frontend-code) describes; `npm run typecheck` runs `vue-tsc`
- **Build tool**: Vite 8
- **Design system**: [@nldd/design-system](https://www.npmjs.com/package/@nldd/design-system) web components
- **Backend**: [editor-api](./editor-api), which also serves the built app
- **Location**: `frontend/`

## Routes and pages

`src/router.ts` is the map. `App.vue` is only a `<router-view>`; the real structure is one persistent shell, `AppShell.vue`, with the Home and Editor sections as its children, plus a handful of top-level pages that carry their own title bar instead of the app chrome.

### The shell

`AppShell.vue` holds the chrome that survives a switch between sections: the Home/Editor tab bar, the search field (a button below the `lg` breakpoint), the "+" menu, the traject menu and the account menu. Switching tabs swaps only the nested `<router-view>`, so the chrome is not rebuilt. In the editor the shell also renders the document tab bar of open articles, and a "PR #N" link to the pull request that the last save in the traject wrote to.

- **"+" menu**: inside a traject it offers *Regelwerk toevoegen*, *Werkdocument toevoegen* (new or upload) and *Leden uitnodigen*. Logged out, the same button opens a prompt to log in or request an account.
- **Traject menu** (`TrajectMenu.vue`, `MobileTrajectSheet.vue` on small screens): switch traject or go to the traject chooser.
- **Account menu**: log in or out, *Account aanvragen*, the color scheme (*Weergave*), *Instellingen*, *Harvester* for users with a harvester role, *Over RegelRecht* and *Help*.

**Search** (`SearchPopover.vue`) goes through the sources in order: the traject's own repository, the example rulework, then the central corpus. It shows the matches of the first source that has any, and a line under the search field names that source. Every other source with matches gets a row (*Ook 3 treffers in …*) that switches the list to that source; the backend caps a search at 60 matches, ordered by source, so the count of the last source can be short and is then shown with a plus. Only when no source matches does it search wetten.overheid.nl, after login; each match there is a link to the official text, and opening it does not fetch the law into the library. Outside a traject, *Alleen verrijkte regelwerken* narrows the search to laws whose current version has a machine version. With an empty field and no login, the popover suggests five laws picked at random from `public/favorites.json`, which the build reads in (`vite.config.ts`).

The editor tab requires a login. Every route marked `requiresAuth` waits for `/auth/status` before it mounts; a user without a session goes to `/uitgelogd`, which keeps the intended destination in `return_url` for the log-in button.

### Home

Every Home route renders `LibraryView.vue`, which uses a navigation split view: a primary sidebar, a secondary sidebar and a main pane. What the panes show depends on the route.

| Route | What it shows |
|-------|---------------|
| `/` | The public corpus (*Corpus juris*). The sidebar lists *Favorieten* and *Recent bekeken*. Favorites need a login; without one the star is not shown. |
| `/corpus-juris/{lawId}/{article?}` | A law from the public corpus: articles in the secondary sidebar, the article in main. A law whose current version is enriched gets *Tekst*, *Machine* and *YAML* tabs under a banner that says the machine version is an example and not established; any other law shows its text only, with a note when another version does have a machine version. |
| `/trajecten/{ref}` | The traject landing. The sidebar adds *Instellingen*, *Werkdocumenten* and *Taken*, and the section *In dit traject*. |
| `/trajecten/{ref}/corpus/{lawId}/{article?}` | A law as the traject has it, read through `/api/trajects/{ref}/corpus/...`. |
| `/trajecten/{ref}/werkdocumenten/{docPath?}` | The traject's werkdocumenten: the document list in the secondary sidebar, the Markdown editor in main. |
| `/trajecten/{ref}/instellingen/{details\|leden}` | Traject settings: *Algemeen* (name, description, status, your role; delete or leave) and *Leden* (members, roles, pending invites). |
| `/trajecten/{ref}/structuur-controle` | The *Algemeen* settings pane with the structure report open as a sheet over it. |
| `/trajecten/{ref}/taken/{categorie?}/{lawId?}` | The user's tasks in the traject, by category. |

A traject ref has the form `{slug}-{8 hex}`. The router pins that pattern in every traject route, and law `$id`s use underscores, so a law id can never be read as a traject ref.

**Werkdocumenten** are Markdown documents stored in the traject's own repository, under `documents/{ref}/`. A new one opens empty in the editor. An upload is converted to Markdown by a pipeline job; before it starts, a confirmation step asks whether a language model may be used for the conversion. Conversions still running show above the document list, and a failed one becomes a task.

**Taken** lists open tasks under *Prioriteit*, *Wachten op* and *Alle taken*, then *Werkdocumenten* and one entry per law that tasks refer to. A category without tasks is not shown. Tasks come from pipeline jobs started for the traject: a harvest, an enrichment or a document conversion ends in a review task, and a job that fails leaves a failure task. A task never opens in the main pane itself; *Beoordelen* goes to the editor or the werkdocument it concerns.

**Structuurcontrole** (`TrajectIntegrityPane.vue`) reports what is wrong with how the traject corpus is laid out, with a remedy per finding. The case that prompted it: the law index reads a law id from the directory name, while the editor switches to the `$id` in the YAML after loading, and when those differ everything after it fails with "not found". The report loads on opening and on *Opnieuw controleren*, not periodically.

**Members** have one of two roles, *Beheerder* (owner) or *Bijdrager* (contributor). An owner changes roles and removes members; the last owner cannot be demoted. Invites go through `InviteMembersSheet.vue`, reachable from the members pane and the "+" menu.

### Editor

`/trajecten/{ref}/editor/{lawId?}/{article?}` renders `EditorView.vue`. There is no editor outside a traject: `/editor`, the old `/editor/{lawId}` links and `/editor.html` all redirect to the traject chooser, carrying the law along so it opens after the choice.

The editor is a side-by-side split view with one pane per view. Each pane has a menu to switch its view and to move it left or right. The pane layout is kept in `localStorage`.

| View | Content |
|------|---------|
| *Tekst* | The article text, editable for users with write access, with a formatting toolbar (bold, italic, list type). Selecting text starts a note. |
| *Machine* | The `machine_readable` section as a structured view. `EditSheet.vue` and `ActionSheet.vue` edit definitions, inputs, outputs and actions. Behind `panel.regelrijm` the article's actions come first, each as Dutch text ([RFC-054](/rfcs/rfc-054)) with the article text that linking notes tie to it; a name leads to the rule that computes it, *Actie openen* opens `ActionSheet.vue`. |
| *Scenario's* | `ScenarioBuilder.vue`: the law's Gherkin scenarios, run in the browser. Per scenario, *Resultaat* opens the execution trace and *Graaf* the cross-law graph with the trace laid over it (`LawGraphView.vue`). |
| *YAML* | The raw YAML in a code editor. A parse error shows under it; if the engine refuses the YAML, a banner says so above the panes. |
| *Notities* | All notes on the article, and the drafts that have not been saved yet. |

A law without a `machine_readable` section shows an empty state in *Machine* and *YAML*, with the option to request enrichment or start writing by hand.

### Top-level pages

| Route | Page |
|-------|------|
| `/trajecten` | `TrajectChooserView.vue`: pick a traject or create one. `?sectie=` and `?law=`/`?article=` carry the intended destination. |
| `/editor/nieuw-traject` | `TrajectCreateView.vue`: the form for a new traject. |
| `/account-aanvragen` | `AccountRequestView.vue`: a public page on who can get an account and how. |
| `/uitgelogd` | `SessionExpiredView.vue`: the public landing after a session ends. |
| `/harvesting/...` | The harvester dashboard; see [Corpusinwinning](#corpusinwinning-harvester-dashboard) below. |

Older paths (`/library/...`, `/corpus-juris`, `/werkdocumenten/{ref}/...`) redirect to their current equivalents.

### Instellingen

The settings sheet (`SettingsSheet.vue`, from the account menu) has two sections, and the menu item disappears when neither applies:

- **Koppelingen**: link or unlink a personal GitHub account, shown when the deployment has a GitHub OAuth app and writes require a personal token.
- **Beheer**, for the `editor-admin` role: the feature-flag switches below, and the switch that makes writes require a personal GitHub account. These settings apply to every user of the deployment.

## Feature flags

Flags are deployment-wide, stored in the `feature_flags` table and served by `GET /api/feature-flags`. `PUT /api/feature-flags/{key}` is behind the `editor-admin` role. The frontend keeps its own defaults in `src/composables/useFeatureFlags.ts` so it can render before the API answers.

The list in `packages/editor-api/src/feature_flags.rs` is also the allow-list: the backend rejects a PUT for a key it does not know with 400, and the frontend then reverts the switch. The two lists therefore have to name the same keys.

| Key | Default | Effect |
|-----|---------|--------|
| `panel.article_text` | on | Offers the *Tekst* pane in the editor. |
| `panel.machine_readable` | on | Offers the *Machine* pane. |
| `panel.scenario_form` | on | Offers the *Scenario's* pane. |
| `panel.yaml_editor` | on | Offers the *YAML* pane. |
| `panel.notes` | on | Offers the *Notities* pane. |
| `panel.regelrijm` | off | Reads the actions in *Machine* as RegelRijm text (experimental). |
| `github.user_oauth` | off | Traject writes require the acting user's own GitHub token, and the settings sheet offers linking a GitHub account. The `GITHUB_USER_TOKEN_REQUIRED` environment variable can force this on regardless of the flag. |

Turning a `panel.*` flag off removes that pane from the editor; turning it back on adds it again. `panel.regelrijm` is the exception: it adds no pane, but changes how *Machine* reads its actions. Other users see the change on their next page load.

When the backend has no database, the PUT returns 503 and the frontend keeps the change in `localStorage`. In a development setup without OIDC a failed write is also kept locally, so the panes stay switchable.

## Data loading

Law content comes from the editor-api. `src/composables/corpusUrls.ts` builds every corpus URL in one of two shapes: `/api/corpus/...` for the public corpus (read-only) and `/api/trajects/{ref}/corpus/...` inside a traject (read and write). `useLaw.ts` loads a law with a `GET` on that URL and saves it with a `PUT`. A save in a traject is written back to the traject's repository; the response carries the pull request it went to, which is where the "PR #N" link comes from.

Scenarios run on the engine compiled to WASM, loaded by `useEngine.ts` from the WASM package that `just wasm-build` writes into the app's public directory.

The `scripts/copy-laws.js` step still runs before `dev` and `build`. It copies local corpus files and the note vocabulary into a `data` folder in the app's public directory; the app reads only the ambiguity vocabulary from there at runtime.

## Design system

The UI is built from `@nldd/design-system` web components (element prefix `nldd-`). `src/main.ts` imports `src/nldd-components.js`, a generated list with one entry point per component the app renders, plus the design system's styles. `script/check-nldd-imports.mjs` keeps that list in step with the tags in the source, so a newly used component fails the build rather than silently missing. A representative slice:

| Category | Components used |
|----------|----------------|
| **Layout** | `nldd-shell`, `nldd-bar-split-view`, `nldd-navigation-split-view`, `nldd-side-by-side-split-view`, `nldd-page`, `nldd-toolbar`, `nldd-container` |
| **Navigation** | `nldd-top-title-bar`, `nldd-tab-bar`, `nldd-document-tab-bar`, `nldd-menu` |
| **Inputs** | `nldd-search-field`, `nldd-text-field`, `nldd-multi-line-text-field`, `nldd-dropdown`, `nldd-combo-box`, `nldd-segmented-control`, `nldd-switch` |
| **Lists & cells** | `nldd-list`, `nldd-list-item`, `nldd-text-cell`, `nldd-icon-cell`, `nldd-collection` |
| **Actions & overlays** | `nldd-button`, `nldd-icon-button`, `nldd-button-group`, `nldd-inline-dialog`, `nldd-modal-dialog`, `nldd-sheet` |
| **Content** | `nldd-rich-text`, `nldd-code-viewer`, `nldd-code-editor`, `nldd-title`, `nldd-tag` |

App-level CSS is small. `frontend/css/main.css` holds a few resets and the `--color-primary` token (`#154273`). Two feature stylesheets sit next to their code: `src/harvester/harvester.css` for the dashboard and `src/components/graph/graph-styles.css` for the law graph.

## Vue components

Besides the views in `src/` (`AppShell.vue`, `LibraryView.vue`, `EditorView.vue` and the top-level pages), `src/components/` holds 40 components. The ones that carry most of the behavior:

| Component | Purpose |
|-----------|---------|
| `ArticleText.vue` / `ArticleTextEditor.vue` | Render and edit article text |
| `MachineReadable.vue` / `MachineEmptyState.vue` | Machine-readable view, and the empty state with enrichment |
| `EditSheet.vue` / `ActionSheet.vue` / `OperationSettings.vue` | Editing definitions, actions and operations |
| `ScenarioBuilder.vue` / `ScenarioForm.vue` | Scenario list and authoring |
| `ExecutionTraceView.vue` / `LawGraphView.vue` (+ `graph/`) | Execution trace and cross-law graph |
| `NoteCreator.vue` / `NoteCard.vue` | Stand-off notes (RFC-018) |
| `RegelRijmPane.vue` / `RegelRijmText.vue` (in `src/regelrijm/`) | The RegelRijm readings at the top of *Machine* and the token text they show (RFC-054) |
| `DocumentList.vue` / `DocumentEditor.vue` | Werkdocumenten |
| `TasksCategoriesPane.vue` / `TasksListPane.vue` | Taken |
| `TrajectDetailsPane.vue` / `TrajectMembersPane.vue` / `TrajectIntegrityPane.vue` | Traject settings and the structure report |
| `SearchPopover.vue` / `AddLawSheet.vue` | Finding a law, and adding one to a traject: copied from the central corpus, harvested by BWB id, or converted from an uploaded PDF or DOCX |

Shared state and API calls live in composables under `src/composables/`, most of them module-level singletons (`useTrajects.ts`, `useTasks.ts`, `useFeatureFlags.ts` and others).

## Corpusinwinning (harvester dashboard)

The harvester dashboard is a section of this app, in `frontend/src/harvester/`. It reaches the standalone [Admin](./admin) API through the editor-api proxy at `/api/harvest-admin/*`, which forwards the session cookie so the admin service applies its own role checks. `packages/admin/` serves no frontend of its own.

The route `/harvesting` requires one of `harvester-reader`, `harvester-writer`, `harvester-admin` or `regelrecht-admin`; a logged-in user without one goes back to `/`. The entry point is *Harvester* in the account menu. The dashboard has its own title bar with a back button that returns to where the user came from, and a button for a new harvest job.

| Tab | Route | Content |
|-----|-------|---------|
| *Overzicht* | `/harvesting/overview` | Totals, a status breakdown and daily chart per job type, and recent failures |
| *Wetten* | `/harvesting/law-entries` | Per law its status and coverage, with actions to harvest, enrich, or reset a law whose retries are exhausted |
| *Taken* | `/harvesting/jobs` | The job queue, grouped per law or as a flat list (`?view=flat`), with a detail sheet per job |
| *Markeringen* | `/harvesting/markings` | Markings that enrichment left in the corpus: places where a construct could not be expressed. Grouped into clusters by the change that would resolve them (a format change or an operation); selecting a cluster filters the list. |
| *Untranslatables* | `/harvesting/untranslatables` | The predecessor of markings, for laws still on schema v0.5.x |

Write actions on this dashboard are enforced by the admin API, not by the frontend.

## Development

```bash
just dev editor    # editor-api + vite on :7300, against Keycloak
just dev           # the same plus the admin API, so the Corpusinwinning section works
```

`just dev` needs `.env.sso-local` (see [Auth and roles](/auth-and-roles/)). Running `npm run dev` in `frontend/` starts only Vite on port 3000 and proxies `/api`, `/auth` and `/health` to an editor-api on port 8000 (`API_PORT` overrides it).

## Deployment

The editor ships as one Docker image (`regelrecht-editor`) that bundles the built frontend with the editor-api binary, which serves the static files and the REST API. It runs on RIG/ZAD:

- **Production**: `editor.regelrecht.rijks.app`
- **PR previews**: for a pull request that carries the `deploy:preview` label
