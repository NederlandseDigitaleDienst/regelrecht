---
title: "Wetsproces Screens"
description: "The screens of the wetsproces server: a public side for citizens and a back office for employees, each built for an instance of its own, with forms that follow from the law and the case's UI spec."
---

`frontend-wetsproces/` holds the screens of the [wetsproces](/components/wetsproces) server. The server builds them into its image and serves them beside its API on one origin, so the session cookie of the login goes along with every call. Each instance serves the build of its [surface](/components/wetsproces#surfaces): the citizens' screens, the back office, or both.

## Overview

- **Language**: TypeScript, Vue 3, the NLDD design system
- **Location**: `frontend-wetsproces/`
- **Served by**: `packages/wetsproces`, from `<WETSPROCES_FRONTEND>/<surface>`
- **Tests**: `npm test -w frontend-wetsproces`; type check with `npm run typecheck -w frontend-wetsproces`

## Screens

| Route | For | What it shows |
|---|---|---|
| `/inloggen` | anyone | The login through Keycloak. In test mode without OIDC, a list of test accounts instead. The citizens' build has it at `/toegang` |
| `/burger` | citizen | The dossier, as chains of what answers to what: an application, the decision on it, an objection to that decision |
| `/burger/aanvraag` | citizen | A new application under the case's own article |
| `/burger/besluit/{id}` | citizen | One decision: what came out, why, and an objection against it |
| `/backoffice` | employee | The executor's submissions without a decision, and the facts it records itself |
| `/backoffice/behandelen/{cel}/{id}` | decider | One submission: the findings the decision asks, a preview, the decision |
| `/backoffice/kroniek/{cel}` | reader | The executor's chronicle |
| `/backoffice/beheer` | admin | The admin panel: per external party the facts it records, what waits for its decision, and its decisions not sent yet |
| `/backoffice/beheer/behandelen/{cel}/{id}` | admin | A submission that waits for an external party, beside the panel, decided on its behalf |
| `/backoffice/klok` | admin, test mode | The test clock |

Both sides are laid out as the [editor](/components/frontend): a bar with the tabs of the side and the account menu, and under it a list in a sidebar with the chosen item in a pane beside it. A decision opens next to the dossier, a submission next to the work queue or the admin panel. The shell is `src/gedeeld/Schil.vue`; the citizen's app uses it too. The appearance submenu in the account menu is the editor's own component, `WeergaveMenuItem.vue` in `frontend-shared`.

The executor is the organization the server works for, and an external party is any other cell of the case; both are explained on [Wetsproces](/components/wetsproces#the-executor-and-the-external-parties). The work queue and the admin panel record a fact with the same sheet (`FeitVastleggen.vue`), and decide in the same pane (`BehandelenPaneel.vue`), which in the panel calls the panel's routes. Each says on screen that the admin acts on the party's behalf.

The tabs and buttons a screen shows follow from `GET /api/ik`, which says what the account may do for the executor and whether it may use the admin panel. That is a matter of what to show: the server checks every call against the roles itself.

The test accounts come from the server (`GET /auth/testpersonen`), derived from the case: two citizens with a made-up number from the BSN test range, a handler and a decider of the executor, and an admin. The login screen says it is a test setup, because anyone who reaches it can pick any account.

Every screen carries a band that says the case, the people and the data are made up, and that an outcome on the screen is computed and is no decision of an authority. A computed decision looks as firm as a real one, and a screenshot travels without the text around it. In test mode the band also gives the date of the test clock.

## Forms from the law

No field of a case is named in the code of the app. A form takes its fields from `GET /api/formulier`, which the kern derives from the law in force on the day of the clock, in the order the law gives them. A yes or no that the law requires is a switch. A yes or no the law lets stay empty is a choice of yes, no or empty, and so is every finding of the authority.

The case's UI spec (`packages/wetsproces/ui_spec.yaml`, read by `packages/poc-chronolexografie/src/ui_spec.rs`) adds to that. A field the system fills shows read-only, or not at all when the spec hides it, and is never sent: the server fills the login field itself. `label` and `help` name a field on the screen and put a line under it, by field name over every form. Without a UI spec every field the law asks is on the form, under its name in the rulework.

```yaml
elements:
  - id: UI1
    element: aanvraagformulier
    fields:
      - name: bsn_aanvrager
        visibility: shown
        fill: {by: system}
        label: Burgerservicenummer
      - name: gevraagde_beschikking
        label: Wat vraagt u aan?
        help: Bijvoorbeeld een gehandicaptenparkeerkaart.
```

With this spec the application form shows the citizen number read-only, with "Ingevuld uit uw inlog." under it, and asks "Wat vraagt u aan?" with the help text below. The other fields carry their names from the rulework, such as "Naam aanvrager".

The form fields and the rules for what the system fills are shared with the chronolexography PoC, in `@regelrecht/frontend-shared/chronolex` ([Shared Frontend Package](/components/frontend-shared)).

Two parts of the UI spec have no effect here yet. A prefill from a source (`fill: {from: ...}`) and the views beside a decision (`answer`) need a route that computes them, and this case uses neither.

## The public side and the back office

`src/burger/` and `src/backoffice/` each have their own routes and their own API client, and every screen loads as a chunk of its own. The citizen's client calls only the routes both sides share and the citizen's own. It does not say how a request reaches the server or proves who sends it: `src/gedeeld/sessie.ts` decides that, with the session cookie in the browser.

Each side has an entry of its own, and one source gives four builds. The mode picks the entry in `vite.config.ts`, and each web build lands in `dist/<surface>/` with an `index.html`:

| Build | Entry | Holds | Output |
|---|---|---|---|
| `npm run build:all` | `index.html`, `src/main.ts` | both sides, for `all` | `dist/all/` |
| `npm run build:burger` | `burger.html`, `src/burgeromgeving/` | the citizens' screens, with the session cookie | `dist/burger/` |
| `npm run build:backoffice` | `backoffice.html`, `src/backofficeomgeving/` | the back office and the login | `dist/backoffice/` |
| `npm run build:app` | `burger.html` | the citizens' screens for [the app](/components/wetsproces-app) | `dist-app/` |

`npm run build` makes the three web builds. The combined build and the back office share the shell (`App.vue`) and the router (`src/router.ts`), which each entry gives the routes of its own sides; neither imports a side itself.

`src/scheiding.test.ts` holds the split. It fails when a module of the public side imports the back office or the login, names a back-office route, or handles a cookie, and when a module of the back office imports the public side or names a citizen's route. It also follows every import from each entry: from the back office's it may reach no module of `src/burger/` or `src/burgeromgeving/`, and from the citizens' none of `src/backoffice/`.

## Running it locally

```bash
npm run build -w frontend-wetsproces   # writes dist/all, dist/burger and dist/backoffice
just wetsproces                        # serves dist/all on http://localhost:7180
```

Without a build the server serves its API alone. To work on the screens, run `just wetsproces` and `npm run dev -w frontend-wetsproces` beside it; the dev server passes `/api` and `/auth` on to port 7180.
