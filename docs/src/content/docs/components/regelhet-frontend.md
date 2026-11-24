---
title: "RegelHet Screens"
description: "The screens of the RegelHet server: a public side for citizens and a back office for employees, each built for an instance of its own, with forms that follow from the law and the case's UI spec."
---

`frontend-regelhet/` holds the screens of the [regelhet](/components/regelhet) server. The server builds them into its image and serves them beside its API on one origin, so the session cookie of the login goes along with every call. Each instance serves the build of its [surface](/components/regelhet#surfaces): the citizens' screens, the back office, or both.

## Overview

- **Language**: TypeScript, Vue 3, the NLDD design system
- **Location**: `frontend-regelhet/`
- **Served by**: `packages/regelhet`, from `<REGELHET_FRONTEND>/<surface>`
- **Tests**: `npm test -w frontend-regelhet`; type check with `npm run typecheck -w frontend-regelhet`

## Screens

| Route | For | What it shows |
|---|---|---|
| `/inloggen` | anyone | The login through Keycloak, see [Logging in through Keycloak](#logging-in-through-keycloak). In test mode without OIDC, a list of test accounts instead |
| `/toegang` | citizen | The citizens' build: the login through Keycloak (see [Logging in through Keycloak](#logging-in-through-keycloak)), or in test mode without OIDC the simulated DigiD |
| `/burger` | citizen | The dossier, as chains of what answers to what: an application, the decision on it, an objection to that decision, the decision on the objection, an appeal to the court and its ruling. An appeal hangs under the decision it contests, which it received from the municipality. A record that neither received nor refers to another, but names one in a field, hangs under that one: the municipality's request to the GGD and the GGD's receipt of it stand under the application they were made for. The GGD's advice and the municipality's receipt of it name no record; the server says which request they answer (`verzoeken` in `GET /api/dossier`), and they stand under that request. Under a decision that granted, a line says how far the acts after it are, in the words of the case, such as "Uw kaart ligt klaar."; each act is a row of the chain |
| `/burger/aanvraag` | citizen | A new application under the case's own article |
| `/burger/dossier/{id}` | citizen | One record of the dossier; every row of the dossier opens one. A submission shows what the citizen filed, with the labels of the UI spec and where the system filled a field, what it is directed against, and what answers it now ([What answers a submission](/components/regelhet#what-answers-a-submission)): the decision that holds, a new decision a decision on objection took in the place of the one it revoked, or that it waits. A revoked or annulled decision stays in view, marked as such. A recorded fact, such as the copy of a ruling, shows what it holds and what it is about. `/burger/besluit/{id}` is the same screen, for links given out before. A decision shows what came out, why, whether it still holds, and what the citizen can file against it: an objection against a decision on an application, an appeal to the court against the decision on that objection. Before filing, the screen gives the last day the decision names for it, and after that day says it is late, with the help the UI spec gives that field (for the parking card, what Awb 6:11 says of a late objection). Which output holds the last day, the server derives from the law (`termijn` of the form): the date that the article naming the moment of the objection or appeal reads that moment against, Awb 6:9 here. A decision of another cell than the case's own goes by its legal character, so a court's ruling is an *Uitspraak*. Under every record, the [timeline](/components/regelhet#the-timeline-of-an-application) of its application (`TijdlijnLijst.vue`): each record a row that opens it, and under them the next steps with who is to act |
| `/backoffice` | employee | The work queue in lists, chosen with `?lijst=`. **Te doen**, the list it opens on, holds everything whose next step is on the executor's side, what waited longest first: the submissions that wait for a decision, as the server gives them (`werkvoorraad`), and the grants whose next act a person records. **Na toekenning** holds each grant whose acts are not all done, with how far it is, and **Wacht op anderen** what waits on the answer to a request (`verzoeken`) or on an act the system takes itself; a submission there is not in Te doen until the answer is in. A submission whose decision a court annulled is marked to decide again. For a handler the row of a grant opens the sheet of the next act ([After a grant](/components/regelhet#after-a-grant)); the facts the executor records on its own sit under the lists |
| `/backoffice/behandelen/{cel}/{id}` | employee | One submission, from every row of the work queue, with what the executor's policy asked of another party for it and whether the answer is in; for a decider also the findings the decision asks, and the decision as it would be taken, beside the button that takes it. That preview comes from the server and runs again when a finding, the chronicle or the test clock changes; when the decision cannot be computed yet, it says what is missing. The preview and the decision taken share one view: the outcome in the order and with the labels of the UI spec, why, and folded under "Meer informatie" the full chain, with the same steps as the decision page of the chronolexography PoC. Under the preview, "Daarna doet het systeem zelf" lists the acts the system would then record itself, such as ordering the card (`automatisch` of the preview). When its decision was annulled, the pane shows that decision as such and decides again after the record that annulled it (`na`). The form and the button show only for a submission the work queue holds, which loads again whenever a submission opens; a submission another employee just decided gets no button. At the bottom, the timeline of the application, the same component as on the citizen's side: a submission of the executor opens here, another record of the executor in its chronicle |
| `/backoffice/kroniek/{cel}` | reader | The executor's chronicle. Every record opens in a sheet with what it holds, also the one `?stuk=<id>` names; a submission, or a decision on one, leads on to its panel in the work queue. A decision of the executor also shows its full chain |
| `/backoffice/beheer` | admin | The admin panel: per external party the requests of the executor it has not answered, the facts it records, what waits for its decision, and its decisions not sent yet. An answer to a request, such as the GGD's advice, is recorded from that request, which fills in for whom it is; without a request only for an applicant with none open, and the sheet says so before it sends one for a person whose request is still open |
| `/backoffice/beheer/behandelen/{cel}/{id}` | admin | A submission that waits for an external party, beside the panel, decided on its behalf |
| `/backoffice/simulatie` | admin, test mode | The simulation, in two parts: the test clock, and the button that empties the test setup after a confirmation that says what goes and that it cannot be undone. `/backoffice/klok` leads here |

Both sides are laid out as the [editor](/components/frontend): a bar with the tabs of the side and the account menu, and under it a list in a sidebar with the chosen item in a pane beside it. Every record of the dossier opens next to it, a submission next to the work queue or the admin panel. The shell is `src/gedeeld/Schil.vue`; the citizen's app uses it too. The appearance submenu in the account menu is the editor's own component, `WeergaveMenuItem.vue` in `frontend-shared`.

The executor is the organization the server works for, and an external party is any other cell of the case; both are explained on [RegelHet](/components/regelhet#the-executor-and-the-external-parties). The work queue and the admin panel record a fact with the same sheet (`FeitVastleggen.vue`), and decide in the same pane (`BehandelenPaneel.vue`), which in the panel calls the panel's routes. Each says on screen that the admin acts on the party's behalf.

The tabs and buttons a screen shows follow from `GET /api/ik`, which says what the account may do for the executor and whether it may use the admin panel. That is a matter of what to show: the server checks every call against the roles itself.

The test accounts come from the server (`GET /auth/testpersonen`), derived from the case: two citizens with a made-up name and a made-up number from the BSN test range, a handler and a decider of the executor, and an admin. The login screen says it is a test setup, because anyone who reaches it can pick any account.

### Logging in through Keycloak

With OIDC the screen that logs in (`/inloggen`, and `/toegang` in the citizens' build) shows nothing to click for someone without a session: it sends the browser on to `/auth/login` at once. The platform's access wall in front of a deployment has logged the person in to Keycloak already, so Keycloak sends them straight back, logged in. The way back runs through the same screen, marked with `na=keycloak`, and from there to the screen of the link. The screen stays where going on would loop or hide what happened (`src/inloggen/stap.ts`):

- Just after logging out it says "U bent uitgelogd", with a button to log in again. The tab remembers the logout in its session storage across the way out through Keycloak (`src/inloggen/uitgelogd.ts`).
- After a failed return it says the login did not succeed. The server answers a failed `/auth/callback` with a redirect to this screen and `?fout=inloggen` (`login_failed_to_screen` in `src/login.rs`), and a browser that comes back marked `na=keycloak` without a session gets the same message, with or without storage.
- For an account without a role on this server it says so and offers to log in with another account, since logging in again would give the same roles. The callback finds that out (a 403 Keycloak did not send, `?fout=rol`), and so does `GET /api/ik` for a session that is in.
- When the server does not answer, it says so and offers to try again, without logging anyone out.

After a failed attempt the button starts again from the start page, not from the link. The simulated DigiD of the test login stays a screen: there is nothing to skip.

In the citizens' build the test login is a simulated DigiD step (`src/inloggen/DigidSimulatieView.vue`): pick one of those citizens from a list, or type a name and a number of the test range. The heading and a banner say it is a simulation and not DigiD, and the screen takes no logo, colour or wording of the real one. The server checks the number and gives the account the citizen's role only ([Login](/components/regelhet#login)). The app gets in by activation and never shows this screen. A link to a screen opened without a session leads to the login first and back to that screen after it, in this build and in the combined one; the citizens' build follows only a path under `/burger/`, the combined build one under `/burger/` or `/backoffice/` (`src/gedeeld/terug.ts`).

When a call fails, a screen shows the server's own reason. An answer without one, such as a page of the platform while the server restarts, or no answer at all, becomes a short line that says the server cannot be reached and that what was filled in stays; the raw body never reaches the screen. `reden` in `src/gedeeld/sessie.ts` does this for both transports.

Every screen carries a band that says the case, the people and the data are made up, and that an outcome on the screen is computed and is no decision of an authority. A computed decision looks as firm as a real one, and a screenshot travels without the text around it. In test mode the band also gives the date of the test clock.

## Forms from the law

No field of a case is named in the code of the app. A form takes its fields from `GET /api/formulier`, which the kern derives from the law in force on the day of the clock, in the order the law gives them. A yes or no that the law requires is a switch. A yes or no the law lets stay empty is a choice of yes, no or empty, and so is every finding of the authority.

The case's UI spec (`packages/regelhet/ui_spec.yaml`, read by `packages/poc-chronolexografie/src/ui_spec.rs`) decides how the screens show those fields, on every form and beside every record: the application, an objection or appeal, the findings of a decision, a fact an employee records, and what a submission, fact or decision holds. The fields stand in the order the spec names them, under the heading of their `group`; a group stands where its first field stands, and a field the spec does not name follows, in the order of the law. `label` and `help` name a field and put a line under it, by field name. The shared code is in `src/gedeeld/formulier.ts` (`geordend`), used by `SpecFormulier.vue` for a form and `Gegevens.vue` for a record. A field the system fills shows read-only with the value the server will record, and under it where that value comes from: the login, the day of submission, a fixed value, or the article of the rule whose outcome it is. The form never sends such a field. The sheet that records a fact does the same for a field the system fills with the day of the clock or a fixed value, and for one the request it answers holds, each in its place under the heading of its group. A field the spec hides is not on the form, also when the system fills it. Without a UI spec every field the law asks is on the form, under its name in the rulework.

For the parking card the application form has, under "Over deze aanvraag":

```yaml
- name: gevraagde_beschikking
  group: Over deze aanvraag
  visibility: hidden
  fill: {by: system, value: gehandicaptenparkeerkaart}
  label: Wat vraagt u aan?
- name: dagtekening
  group: Over deze aanvraag
  visibility: shown
  fill: {by: system, submission: date}
  label: Dagtekening
- name: aanvraag_ondertekend
  group: Over deze aanvraag
  visibility: hidden
  fill:
    by: system
    from: beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#8#aanvraag_geldt_als_ondertekend
    parameters: {inlogmiddel: login_method}
  label: Ondertekend
```

A citizen sees only the date, read-only, with the day of submission under it. The requested decision and the signature are hidden: the server fills and records them all the same. A hidden field is out of sight on the form and nowhere else. The application holds it, so the dossier and the chronicle show its value. The forms still say how the system fills it (`systeem`), and the citizen's dossier and the back office's handling of an application name that origin beside the value, for the signature the article of the municipality's policy. Where the values come from is on [RegelHet](/components/regelhet#what-the-server-fills-in-an-application).

The form fields are shared with the chronolexography PoC, in `@regelrecht/frontend-shared/chronolex` ([Shared Frontend Package](/components/frontend-shared)).

The server computes no prefill the applicant may change (`fill: {from: ...}` without `by: system`), and refuses to start with a UI spec that has one, so a form never shows such a field empty without a reason. The views beside a decision (`answer`) have no effect here yet; they need a route that computes them, and this case uses none.

## The public side and the back office

`src/burger/` and `src/backoffice/` each have their own routes and their own API client, and every screen loads as a chunk of its own. The citizen's client calls only the routes both sides share and the citizen's own. It does not say how a request reaches the server or proves who sends it: `src/gedeeld/sessie.ts` decides that, with the session cookie in the browser.

Each side has an entry of its own, and one source gives four builds. The mode picks the entry in `vite.config.ts`, and each web build lands in `dist/<surface>/` with an `index.html`:

| Build | Entry | Holds | Output |
|---|---|---|---|
| `npm run build:all` | `index.html`, `src/main.ts` | both sides, for `all` | `dist/all/` |
| `npm run build:burger` | `burger.html`, `src/burgeromgeving/` | the citizens' screens, with the session cookie | `dist/burger/` |
| `npm run build:backoffice` | `backoffice.html`, `src/backofficeomgeving/` | the back office and the login | `dist/backoffice/` |
| `npm run build:app` | `burger.html` | the citizens' screens for [the app](/components/regelhet-app) | `dist-app/` |

`npm run build` makes the three web builds. The combined build and the back office share the shell (`App.vue`) and the router (`src/router.ts`), which each entry gives the routes of its own sides; neither imports a side itself.

`src/scheiding.test.ts` holds the split. It fails when a module of the public side imports the back office or the login, names a back-office route, or handles a cookie, and when a module of the back office imports the public side or names a citizen's route. It also follows every import from each entry: from the back office's it may reach no module of `src/burger/` or `src/burgeromgeving/`, and from the citizens' none of `src/backoffice/`.

## Running it locally

```bash
npm run build -w frontend-regelhet   # writes dist/all, dist/burger and dist/backoffice
just regelhet                        # serves dist/all on http://localhost:7180
```

Without a build the server serves its API alone. To work on the screens, run `just regelhet` and `npm run dev -w frontend-regelhet` beside it; the dev server passes `/api` and `/auth` on to port 7180.
