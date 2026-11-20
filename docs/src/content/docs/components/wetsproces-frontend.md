---
title: "Wetsproces Screens"
description: "The screens of the wetsproces server: a public side for citizens and a back office for employees, with forms that follow from the law and the case's UI spec."
---

`frontend-wetsproces/` holds the screens of the [wetsproces](/components/wetsproces) server. The server builds them into its image and serves them beside its API on one origin, so the session cookie of the login goes along with every call.

## Overview

- **Language**: TypeScript, Vue 3, the NLDD design system
- **Location**: `frontend-wetsproces/`
- **Served by**: `packages/wetsproces`, from the directory in `WETSPROCES_FRONTEND`
- **Tests**: `npm test -w frontend-wetsproces`; type check with `npm run typecheck -w frontend-wetsproces`

## Screens

| Route | For | What it shows |
|---|---|---|
| `/inloggen` | anyone | The login through Keycloak. In test mode without OIDC, a list of test accounts instead |
| `/burger` | citizen | The dossier, as chains of what answers to what: an application, the decision on it, an objection to that decision |
| `/burger/aanvraag` | citizen | A new application under the case's own article |
| `/burger/besluit/{id}` | citizen | One decision: what came out, why, and an objection against it |
| `/backoffice` | employee | Per cell the employee works for, the submissions without a decision and the facts the cell records |
| `/backoffice/behandelen/{cel}/{id}` | decider of the cell | One submission: the findings the decision asks, a preview, the decision |
| `/backoffice/kroniek/{cel}` | reader of a cell | The chronicle of a cell |
| `/backoffice/klok` | admin, test mode | The test clock |

Both sides are laid out as the [editor](/components/frontend): a bar with the tabs of the side and the account menu, and under it a list in a sidebar with the chosen item in a pane beside it. A decision opens next to the dossier, a submission next to the work queue. The shell is `src/gedeeld/Schil.vue`; the citizen's app uses it too. The appearance submenu in the account menu is the editor's own component, `WeergaveMenuItem.vue` in `frontend-shared`.

The tabs and buttons a screen shows follow from `GET /api/ik`, which says per cell what the account may do. That is a matter of what to show: the server checks every call against the roles itself.

The test accounts come from the server (`GET /auth/testpersonen`), derived from the cells of the case: two citizens with a made-up number from the BSN test range, a handler and a decider for each cell, and an admin. The login screen says it is a test setup, because anyone who reaches it can pick any account.

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

`src/burger/` and `src/backoffice/` each have their own routes and their own API client, and every screen loads as a chunk of its own. The citizen's client calls only the routes both sides share and the citizen's own. It does not say how a request reaches the server or proves who sends it: `src/gedeeld/sessie.ts` decides that, with the session cookie in the browser. A test (`src/scheiding.test.ts`) fails when a module of the public side imports the back office or the login, names a back-office route, or handles a cookie.

The split lets the public side be bundled on its own, from `burger.html`, with a transport chosen per build: the commands of [the app](/components/wetsproces-app)'s Rust core, or HTTP with the session cookie.

## Running it locally

```bash
npm run build -w frontend-wetsproces   # writes frontend-wetsproces/dist
just wetsproces                        # serves it on http://localhost:7180
```

Without a build the server serves its API alone. To work on the screens, run `just wetsproces` and `npm run dev -w frontend-wetsproces` beside it; the dev server passes `/api` and `/auth` on to port 7180.
