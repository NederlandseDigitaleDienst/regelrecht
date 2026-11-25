---
title: "RegelHet"
description: "A server that runs one legal process on the cells of chronolexography for many users at once, behind a login through Keycloak, with its chronicles in memory or in PostgreSQL."
---

RegelHet is a server that runs one legal process, from application to decision and on to objection and appeal, for everyone who uses it. It is one implementation built on the regelrecht method; another system can run the same cells in its own way. The process steps are those of the chronolexography PoC; this component adds what a shared server needs on top of them.

## Overview

- **Language**: Rust (Axum)
- **Location**: `packages/regelhet/` (crate `regelrecht-regelhet`)
- **Intended URLs**: `regelhet.regelrecht.rijks.app` for citizens and `regelhet-backoffice.regelrecht.rijks.app` for employees (not deployed yet)
- **Port**: 8000
- **Process**: the disabled parking card of the fictitious municipality of Wilgendaal, from `packages/regelhet/bron.yaml`

## What it runs

The server loads exactly one source manifest at startup, the same `bron.yaml` format the [PoC portal](/components/poc-portal) serves sources from, read by the same code (`packages/corpus/src/source_manifest.rs`). The server reads it from a local path (`REGELHET_MANIFEST`) and fetches nothing itself. In the image that is the manifest the image ships with; a deployment can put another case at that path, as the Helm chart does: it unpacks a copy of the case into `/case`, or clones the case repository there before the server starts. That repository may be on GitHub or elsewhere. A manifest may make several cases, one per case file under `casussen/`, so that one repository holds several processes. `REGELHET_CASUS` then names the one this server runs. Without it the manifest must make exactly one case. The server refuses to start when the manifest makes none, when it makes several and none is chosen, when it makes none by the chosen name, and when any of its cases does not load.

The first case is the disabled parking card (BABW, article 49). It runs three cells: the municipality that decides, the GGD that examines the applicant and advises, and the administrative court that decides on an appeal. The regelwerken and cells come from the PoC's corpus; only the case file in `packages/regelhet/casussen/` and the UI spec `packages/regelhet/ui_spec.yaml` belong to this component. The server takes the case's own login field (`inloggen.veld`) as the field that identifies a citizen, or else the one field its UI spec has the login fill (`fill: {by: system}` without a source). Here that is `bsn_aanvrager`, marked in the UI spec. The server refuses to start when it finds no such field or more than one.

## The executor and the external parties

One cell is the executor: the organization this server runs the process for, and the only one whose employees log in to the back office. It is the case's own cell, which the case file names under `cel`; for the parking card that is the municipality. No setting of the server names it, so another case brings its own executor.

The other cells of the case stay in the process, because the steps need their chronicles, but nobody works for them in this server. Modeling them as organizations with employees of their own would not end: the GGD depends on other parties in turn, and those on others. What such an external party does comes in through the admin panel instead. An admin (`regelhet-admin`) enters there, on the party's behalf, what the process allows of it at that moment: the GGD records its advice, the court decides on an appeal and sends a copy of its judgment to the municipality. The panel uses the same forms as the rest of the back office, derived from the law and the UI spec.

What the executor sees of an external party follows the process. The municipality reads what the GGD sends it, which lands in the municipality's own chronicle, and not the GGD's chronicle. The admin panel shows of a party only what an input needs: the submissions that wait for its decision, and its decisions of which no other cell has received a copy. Neither the panel nor any route of the back office returns a party's chronicle.

The admin panel exists in test mode and in stable mode, because until there is a real link with such a party it is the only way its acts reach the process. A party can also run on a server of its own, linked with this one: its staff log in there and see their own requests, and the two servers exchange what each has to know as signed messages. The panel then points to that party's own system, and takes input for it only as a fallback in test mode. [A link between servers](/components/regelhet-koppeling) describes it; it carries facts, so a party that decides, such as the court, stays in the panel.

## A request to an external party and the answer back

Two facts of one article with the same parameters, each established by another cell, form a message: when one cell records the first, the other cell records the second, its receipt, in its own chronicle, in the same step and with the same values. Nothing reads across. The receiving cell answers from its own chronicle, and no letter is simulated beyond those two records. The kern delivers every fact this way (`afleveren` in `kern.rs`), in whichever direction the article points.

For the parking card it runs both ways:

| Direction | Article | The sender records | The receiver records |
|---|---|---|---|
| The municipality asks the GGD for an examination | 9 of the municipality's fictitious policy | `geneeskundig_onderzoek_aangevraagd` | `verzoek_om_geneeskundig_onderzoek_ontvangen` |
| The GGD sends its advice back | 5 of the same policy | `geneeskundig_advies_uitgebracht` | `geneeskundig_advies_ontvangen` |

The request comes from the executor's policy. The case file names, under `bij_aanvraag`, the articles the server runs when an application comes in, with what the system knows of it:

```yaml
bij_aanvraag:
  - artikel: beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#9
    parameters:
      datum_verzoek: date        # the day the application came in
      aanvraag: submission       # the id of the application
    antwoord:
      feit: beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht
      datum: datum_onderzoek
```

The article gets the fields of the application as well. Its one output, `geneeskundig_onderzoek_aanvragen`, says whether the college asks. It does not while a request about the same applicant is open, nor when the college gave the applicant a card before, since the college may then forgo the examination (article 2, second paragraph, of the Regeling gehandicaptenparkeerkaart). Both come from the municipality's cell, through article 10, which binds them to its answers. When the article says yes, the request and its receipt at the GGD land in the commit of the application. The store keeps `beleid` as their `recorded_via` and the article as their `recorded_by`, so the chronicle shows that the server recorded them by policy and not at anyone's request.

A request stays open until an answer belongs to it: the fact under `antwoord`, recorded by the cell that received the request, with the same values in the fields both hold (here `bsn_aanvrager`) and a day under `datum` on or after the day of the request. A second application from the same applicant while the request is open asks nothing again, and a retry under the same `Idempotency-Key` gets the first answer without recording anything.

The admin panel lists the open requests per party. The admin answers from such a request: the body of `POST /api/backoffice/beheer/vastleggingen` names the request's receipt under `verzoek`, and the server takes the fields the request holds from it, so the form does not ask for the citizen number. The answer is refused when it is another fact than the one under `antwoord`, when it names another applicant, or when its examination is dated before the request. Once answered, the request leaves the list, and the advice reaches the municipality as it always did.

An advice without a request stays possible, for an applicant about whom no request is open. That covers the applicant who had a card before: the policy asks nothing then, but the college may still want an examination. The panel keeps that path in reserve, below the requests, and the form asks for the citizen number. With a request open for that applicant, the server refuses the advice without it, so an answer cannot miss the request it belongs to.

The back office reads the executor's requests through `GET /api/backoffice/verzoeken`: per request the municipality's record, the party it went to, whether it is open, and the municipality's receipt of the answer. It never returns the GGD's records. The employee sees in the timeline of the application that the examination was asked for and whether the advice is in; a request made for an earlier application of the same person, which a new one waits on as well, has a list of its own under what was submitted. The citizen's dossier holds both records of the request, at the municipality and at the GGD, because both name the applicant.

## How a request runs

Where the PoC keeps one visitor's chronicle in the browser, this server keeps every cell's chronicle in a chronicle store. Each step loads all cells of the case from the store at one moment, runs the step of the PoC's kern (`packages/poc-chronolexografie/src/kern.rs`, used as a library), and writes what the step recorded back in one commit. A step can record in more than one cell: the GGD's advice lands in the GGD's chronicle and in the municipality's.

The store (`ChronicleStore` in `src/store.rs`) loads a chronicle per cell and appends to several cells at once, all or nothing. Each cell in a commit names the head it was read at, the number of records in it then. When another request appended in between, the commit is refused and the server runs the step again on the new state. After three runs in all it gives up and answers `409`. A cell the step only read is part of the commit as well, so a decision taken on an outdated view of the GGD's chronicle is refused too.

A writing request may carry an `Idempotency-Key` header. Sent again by the same account with the same key and the same request, it gets the first answer and nothing is recorded twice. The same key with a different request, or from another account, is refused with `409`. Without the header, every request records anew and the store keeps no answer for it.

What day it is comes from a clock (`Clock` in `src/clock.rs`). Every step runs on that day and the cell records it as the day of the record. In test mode the day an admin set lives in the store, beside the chronicles, so every instance on one database runs on the same day.

## Storage

Two stores implement `ChronicleStore`. The store in memory starts empty and is gone when the server stops. The PostgreSQL store (`src/store/postgres.rs`) keeps the chronicles in a schema of its own, `wetsproces`, in the database it is given. The schema kept that name through the rename to RegelHet: the chronicles of every running process are in it, and sqlx refuses to start when a migration it applied earlier has changed, so the migrations kept their text as well. Its migrations are in `packages/regelhet/migrations/`, and sqlx's migration table sits in the same schema. They are independent of the pipeline's migrations and touch no table in `public`.

| Table | Holds |
|---|---|
| `grams` | One row per record: the cell, its sequence number in that cell (from 1), the record's id (unique across all cells), a format version, the record as `jsonb`, when it was stored, `recorded_by`, the account whose request recorded it, and `recorded_via`, the channel it came in by: `keycloak`, `testlogin`, `fsc4b`, `beheerpaneel` for what an admin entered for an external party, `beleid` for what the server recorded by the executor's policy, or `koppeling` for what the server of another cell sent (migration `0010_link.sql`) |
| `heads` | Per cell its head |
| `receipts` | Per idempotency key the request, with the account that sent it, and the answer |
| `sessions` | The login sessions of both surfaces, made by the session store (`tower-sessions`) at startup; each carries the surface it was made on |
| `test_clock` | In test mode, the day an admin set the clock to; no row until it is first set |
| `fsc4b_challenges`, `fsc4b_sessions` | The challenges and sessions of [FSC voor Burgers](/components/regelhet-fsc4b) |

`recorded_by` sits beside the record, not inside it: the record says what the law made happen, the column says who asked for it. It holds the Keycloak `sub`, or the account of an app (`fsc4b:` and a key thumbprint). `recorded_via` says which channel: `keycloak`, `testlogin` or `fsc4b`. A record the executor's policy made without a person, such as the request to the GGD or the order of a card, has `beleid` there and the article in `recorded_by` (migration `0009_recorded_via_policy.sql`, see [A request to an external party](#a-request-to-an-external-party-and-the-answer-back)). Both are empty for what the server records itself, such as the case's beginning; `recorded_by` is also empty for records stored before the login existed (migration `0002_recorded_by.sql`), and `recorded_via` for those stored before migration `0003_recorded_via.sql`.

`grams` is append-only, and the database enforces it: a trigger refuses every `UPDATE`, `DELETE` and `TRUNCATE` on it. A request that holds the character U+0000 is refused with `422` in both stores, because `jsonb` cannot store it. A record written in a format version this build does not know is not read; the request fails with `500`, and the server log names the cell, the record and the version.

A commit is one transaction. It claims its idempotency key first, so a retry under the same key waits for the first request and then gets its answer. Then it locks the head rows of its cells in order of name, so that two commits never wait on each other in a circle, and checks every head before it appends. Both stores pass one shared set of contract tests (`src/store/contract.rs`), which includes concurrent commits on one cell.

## Modes

`REGELHET_MODE` selects the mode and has no default, not even in the image. Without it the server does not start. The mode sets the clock; where the chronicles live is chosen apart from it, with `REGELHET_STORE`.

- `test` uses a clock an admin sets with `PUT /api/backoffice/klok`, starting on the case's `speeldatum`. An admin can also empty the test setup with `POST /api/backoffice/testomgeving/leegmaken`: the records of every cell go, what the admin panel entered for an external party included, with the answers kept under idempotency keys and the day of the test clock, which then stands on the `speeldatum` again. The sessions of the login and of FSC voor Burgers stay, so nobody is logged out; a citizen still logged in sees an empty dossier. It happens in one transaction that first locks the tables in the order a commit takes them. The same transaction records what the case begins with, as at a fresh start, so no request finds the setup empty. A submission whose step read the chronicles before the reset and commits after it is refused by the store: a commit names the generation it read in, and a reset starts a new one. The server then runs the step again on what the reset left, and the submission lands in the emptied setup. On PostgreSQL the chronicles stay append-only: migration `0007_test_reset.sql` lets a `TRUNCATE` through only in a transaction that set `regelhet.leegmaken` itself, which only this route does in the server. Since migration `0008_reset_generation_and_stable.sql`, a server in stable mode marks its database when it starts, and from then on the triggers refuse that `TRUNCATE` as well. Both guard against a mistake in the server, not against a database user who can run SQL. The server log names the account that emptied the setup. The store is `memory` unless `REGELHET_STORE=postgres` asks for PostgreSQL, which gives a lasting chronicle with a clock that can still be moved. On PostgreSQL the day survives a restart, as the chronicles do. With the OIDC variables set it logs in through Keycloak; without them it offers the test login.
- `stable` uses the real date in the Netherlands, in the Europe/Amsterdam time zone, always the PostgreSQL store, and always the login through Keycloak. It refuses to start without the OIDC variables, `OIDC_REQUIRED_ROLE` or `BASE_URL`, and with `REGELHET_STORE=memory`. It also refuses to start without `REGELHET_SURFACE`, or with `all`. In either mode, an OIDC variable set without `OIDC_CLIENT_ID` stops the server, so a half-finished configuration never opens the test login.

| Variable | Default | Meaning |
|---|---|---|
| `REGELHET_MODE` | none | `test` or `stable`. Required. |
| `REGELHET_SURFACE` | `all` in test mode, none in stable mode | Which side the instance serves: `burger`, `backoffice` or `all` (see [Surfaces](#surfaces)) |
| `REGELHET_STORE` | `memory` in test mode, `postgres` in stable mode | Where the chronicles live |
| `DATABASE_URL` | none | The database for the `postgres` store; `DATABASE_SERVER_FULL` is read as fallback. Required with that store. |
| `REGELHET_DB_MAX_CONNECTIONS` | `5` | How many connections an instance holds to that database at most. The chronicles, the sessions and FSC voor Burgers share one pool. At least `2`: the migration holds its lock on one connection while it migrates on another. The startup line names the value. |
| `REGELHET_MANIFEST` | `packages/regelhet/bron.yaml` | The source manifest |
| `REGELHET_CASUS` | none | The case to run from a manifest that makes several, by the name of its case file: `bedrijfsparkeervergunning` for `casussen/bedrijfsparkeervergunning.yaml`. The error at startup lists the names the manifest makes. The front of an organisation runs no case and refuses it |
| `REGELHET_PORT` | 8000 | The port |
| `REGELHET_FRONTEND` | the build output of `frontend-regelhet/` in the checkout; `/app/frontend` in the image | The built screens, one build per surface in a directory of its name. The server serves `<dir>/<surface>`, and its API alone when there is no `index.html` there |
| `REGELHET_LOGIN` | none | `testlogin` or `oidc`. Without it the OIDC variables decide (see [Login](#login)) |
| `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `OIDC_DISCOVERY_URL` | none | The Keycloak client; `KEYCLOAK_BASE_URL` with `KEYCLOAK_REALM` may replace the discovery URL. Required in stable mode. |
| `OIDC_REQUIRED_ROLE` | none | The role every account needs to log in, `regelhet-toegang`. Required with OIDC; a role without the `regelhet-` prefix is refused, because the realm is shared with the editor. |
| `BASE_URL` | none | Where the server is reached, for the redirect back after login. Required in stable mode. |
| `REGELHET_SUBJECT_CLAIM` | the case's login key, or `bsn` | The claim that identifies an applicant. Without it the case decides: the key of its login (`inloggen.sleutel`, such as `kvk_nummer`), or `bsn` for a case without one. With Keycloak or FSC voor Burgers it must be set when that key is not `bsn`, and it may not differ from a key the case names. The front of an organisation uses `bsn` unless it is set |
| `REGELHET_ACCENT_COLOR` | none | The accent colour of the screens: the name of an NLDD design-system theme, such as `paars` or `donkergroen`. The server hands it to the screens on `GET /api/uiterlijk`, a route without a login, and they set it as `data-nldd-theme`. A name that is not a theme stops the server at startup. Without it the screens wear `donkerblauw` |
| `REGELHET_TITLE` | the case's `titel` | The title in the bar at the top of the screens, such as `Parkeervergunningen`: at most 80 characters on one line. The server hands it to the screens on `GET /api/uiterlijk` as `titel`, with the case's own `titel` when this is not set. A longer title, or one spread over lines, stops the server at startup, and so does a title on the front of an organisation, whose bar carries the organisation's name |
| `REGELHET_LINK_PEERS`, `REGELHET_LINK_PARTY`, `REGELHET_LINK_KEY_FILE` or `REGELHET_LINK_KEY` | none | The link with the server of another cell of the case; see [A link between servers](/components/regelhet-koppeling) |

When the PostgreSQL store is chosen and the database cannot be reached or migrated, the server stops with that error. It does not fall back to memory.

## Surfaces

Citizens and employees each have their own deployment, with its own address and login. Both run the same image on the same database. `REGELHET_SURFACE` says which one an instance is. What differs between them is decided in one place, `src/surface.rs`.

| | `burger` | `backoffice` | `all` |
|---|---|---|---|
| Routes | the shared routes, the citizen's routes, and FSC voor Burgers when configured | the shared routes and those under `/api/backoffice/` | all of them |
| Roles a session keeps | `regelhet-burger` only | every role but `regelhet-burger` | all |
| Session cookie | `regelhet_burger_session` | `regelhet_backoffice_session` | `regelhet_session` |
| Test accounts | the citizens | the employees and the admin | all |
| Screens | `<REGELHET_FRONTEND>/burger` | `<REGELHET_FRONTEND>/backoffice` | `<REGELHET_FRONTEND>/all` |

A route of the other side is not registered at all, so it answers `404` like any unknown path, also to a session that carries every role. The shared routes are `/health`, the login under `/auth/`, and `/api/casus`, `/api/formulier`, `/api/klok` and `/api/ik`.

On the instance for citizens a request never acts with an employee's role, whatever the token carried. On the back office an account with only the citizen's role holds no role of this server and gets `403` on every route the back office has under `/api/`. The server drops the roles a surface does not admit when a request reads its session, and the session store drops them again each time it stores a session, after a login and after a refresh of the token. The test login and Keycloak go through the same filter.

Both instances keep their sessions in `regelhet.sessions`. Each session carries the surface it was made on, and an instance reads a session of another surface as no session: a cookie from the back office, sent to the instance for citizens under that instance's cookie name, gets `401`. A session from before the surfaces existed carries no surface and is read as none either.

The chronicles and the test clock are shared. A citizen's application on the instance for citizens is in the municipality's chronicle on the back office straight away, and the day an admin sets on the back office is the day the next submission is recorded on.

`all` serves both sides on one address, as one server did before the split. It exists so that `just regelhet` stays one command, and stable mode refuses it. FSC voor Burgers serves citizens, so the configuration refuses that channel on `backoffice`.

## Login

Every route except `/health` and `/auth/...` needs a session. With OIDC configured, `/auth/login` logs in through Keycloak with the shared `packages/auth` crate, as the editor does. The roles and their Keycloak setup are on [Authentication & Roles](/auth-and-roles#regelhet-roles).

Sessions live with the chronicles: in memory with the store in memory, in `regelhet.sessions` with PostgreSQL. The cookie has a name per surface (see [Surfaces](#surfaces)), so it never mixes with the editor's sessions or with the other instance's.

In test mode without OIDC, `POST /auth/testlogin` gives a session with the roles and claims in its body, without Keycloak:

```bash
curl -c jar -H 'content-type: application/json' localhost:7180/auth/testlogin \
  -d '{"sub": "burger-1", "rollen": ["regelhet-burger"], "claims": {"bsn": "999990019"}}'
```

The session it writes has the same keys as one after Keycloak, so every request after it goes through the same checks. The route exists only in that case; with OIDC configured, and always in stable mode, it answers `404`.

The citizens' screen to get in (the simulated DigiD) sends a citizen's number and name instead, and nothing else:

```bash
curl -c jar -H 'content-type: application/json' localhost:7180/auth/testlogin \
  -d '{"bsn": "999990032", "naam": "Anna Voorbeeld"}'
```

The server makes that account itself: the role `regelhet-burger` and no other, the number under the subject claim, and the name in the session, where `GET /api/ik` gives it. It takes only a number of nine digits that passes the eleven test and starts with `99999`, the range made for testing, so a number typed into the simulation is never a real one. A refused number answers `400` with the reason in Dutch. A body that adds roles, claims or a `sub` to the number and name fits neither shape and is refused. The back office serves no citizens and answers `404`.

A case that identifies an applicant by its KvK number (`inloggen.sleutel: kvk_nummer`) gets the same login as a business, as a simulated eHerkenning: the body is `{"nummer": "99990011", "naam": "Bakkerij De Wilg"}` (`nummer` and `bsn` name the same field), the number is eight digits, and the account's `sub` is `bedrijf-<number>`. `GET /auth/testpersonen` then offers the numbers of that key in what the case's cells record before the first step, with the name a record beside them gives (the case's `inloggen.naam`, else `handelsnaam`, `statutaire_naam` or `naam`), such as the businesses of a Chamber of Commerce cell. `GET /auth/simulatie` tells the screen which number the case asks for: its claim, label and number of digits.

A case can also log in through a login step of its own (`inloggen.inlogstap`), as eHerkenning does: the applicant gives a user name of the accredited service and the password of the simulation, `{"gebruikersnaam": "bakker", "wachtwoord": "welkom"}`. The service's cell checks its register, records the authentication the case requires (`inloggen.feit`) in a commit of its own, and asserts the KvK number and the name the session gets. A refused login records nothing and answers `400` with the reason. The simulated login of such a case takes no number, because without the step nothing records the authentication and the application would be left undecided. The test login with an account and its claims, meant for scripts, still does. `GET /auth/simulatie` lists the user names under `inlogstap`.

With Keycloak the same case logs in on the token's claim of that key, and the step runs once per session, the first time the citizen reaches a route of the citizens' side. The Keycloak user name (`preferred_username`, which the server keeps beside the subject claim) is the user name at the step, with the password of the simulation. The step must assert the number the token carries. A refusal records nothing, and the application then lacks the authentication, as after a refused login. Another number than the token's is a mistake in the realm: the server logs it, and the authentication the step recorded is that of the step's user, not of the token's number. Either outcome is noted in the session, so the step does not run again on every request. The Fundament plugin gives such a realm its test accounts, under the step's user names.

`GET /auth/logout` beside the test login ends the session and sends the browser to `/`, where the screen to get in waits again.

`REGELHET_LOGIN` overrides that choice. `testlogin` gives the test login in test mode even when the OIDC variables are set, and reads none of them. A ZAD preview needs this: its `keycloak` service sets those variables in the container, while the realm may not have the RegelHet roles yet. Stable mode refuses `testlogin`. `oidc` refuses to start without the OIDC variables, and any other value stops the server too. The line `listening on 0.0.0.0:<port>` at startup names the login, the mode, the surface, the store and its pool size (`db_max_connections`), so the log shows what runs. So does `GET /auth/testpersonen`, which lists the accounts the login screen offers, derived from the case: on `burger` the citizens, on `backoffice` a decider of the executor and the admin.

An app can use the citizen routes without a browser through [FSC voor Burgers](/components/regelhet-fsc4b), when that channel is configured. [The app](/components/regelhet-app) does so with the citizen's screens in a webview.

A process can also belong to an [organisation](/components/regelhet-organisatie): a front with one login before several processes, which hands a person over to this server with a signed statement at `GET /auth/organisatie`.

## API

The keys in requests and answers are those of the kern, as the PoC uses them (`velden`, `bevindingen`, `op`). Which of the routes below an instance has depends on its surface. The routes under `/api/backoffice/` exist on `backoffice` and `all`, those under `/api/dossier` and `/api/indieningen` on `burger` and `all`, and the others everywhere. An error answers `{ "fout": "<reason>" }`. For a `500` the reason is a fixed sentence and the cause goes to the server log only. A body that does not parse gets axum's own plain-text answer. Without a session the answer is `401`; with a session that lacks the role, `403`.

| Method and path | Who | What it does |
|---|---|---|
| `GET /health` | anyone | `ok` once the server runs |
| `GET /api/casus` | citizen or employee | The case and today's date |
| `GET /api/formulier` | citizen or employee | The forms of the process today, with what the UI spec says of each field |
| `GET /api/klok` | citizen or employee | Today's date, and whether it can be set |
| `GET /api/ik` | citizen or employee | Who is logged in, what the account may do for the executor (`cellen`), and whether it may use the admin panel (`partijen_invoeren`) |
| `GET /api/dossier` | citizen | The records about the citizen, from every cell, with their standing (`stand`, [below](#what-answers-a-submission)), under `vervolg` how far the acts after each of their grants are, and under `verzoeken` per request of the policy the ids of its records in the dossier (the request, its receipt, the answer and the receipt of that), so a screen can hang the answer under its request |
| `GET /api/dossier/waarom/{id}` | citizen | Why a decision in the citizen's dossier came out as it did, without how a cell read its chronicle or any record outside the dossier |
| `GET /api/dossier/tijdlijn/{id}` | citizen | The [timeline](#the-timeline-of-an-application) of the application a record of the dossier belongs to, kept to the dossier |
| `GET /api/dossier/uitkomsten/{id}` | citizen | What a submission of the dossier gives under its own article (`kern::wat_volgt`), values only: what its [acknowledgement](#messages-to-the-citizen) says |
| `POST /api/indieningen` | citizen | A submission: `artikel` (the case's own when left out), `op`, `velden` |
| `GET /api/backoffice/cellen/{cel}/kroniek` | reader | The executor's chronicle, with its head and its standing (`stand`); an external party's answers `403` |
| `GET /api/backoffice/werkvoorraad` | reader | The executor's submissions that wait for a decision (`open`), each with the record to decide after (`na`) when its last decision was revoked |
| `GET /api/backoffice/waarom/{id}` | reader | Why a decision of the executor came out as it did, with its chain of steps (`keten`); nothing of another cell's own chronicle but what reached the executor |
| `GET /api/backoffice/tijdlijn/{id}` | reader | The [timeline](#the-timeline-of-an-application) of the application a record of the executor belongs to: the executor's own records, and of another cell no more than the id of what reached the executor |
| `GET /api/backoffice/uitkomsten/{id}` | reader | What a submission of the executor gives under its own article, as for the citizen |
| `POST /api/backoffice/voorvertoning` | decider | What a decision (`op`, `na`, `bevindingen`) on a submission of the executor would come to, with why and its chain, read as for `waarom`, and under `automatisch` what the system would then do itself; under `stil` the findings a policy leaves to the authority for this case, and under `mogelijk` every finding such a policy may leave, reached or not; records nothing |
| `GET /api/backoffice/verzoeken` | reader | The requests the executor's policy made, each with the party, whether it is open, and the executor's receipt of the answer |
| `POST /api/backoffice/vastleggingen` | decider | A fact the executor records: `cel`, `feit`, `velden`, `verwijst_naar` |
| `GET /api/backoffice/vervolg` | reader | Every grant and how far the acts after it are (see [After a grant](#after-a-grant)), and whom it is for: the login field (`aanvrager`) and, where the UI spec fills a field from the name of the login, that name as the application recorded it (`naam`, else null) |
| `POST /api/backoffice/toezendingen` | decider | A copy of the executor's decision `besluit` to the cell it concerns |
| `POST /api/backoffice/besluiten` | decider | A decision on a submission of the executor: `op`, `na`, `bevindingen` |
| `PUT /api/backoffice/klok` | admin | In test mode, set the date with `{ "datum": "YYYY-MM-DD" }` |
| `POST /api/backoffice/concepttoelichting` | decider | A draft explanation of the executor's decision `besluit` in plain language, by Vlam (see [A draft explanation by Vlam](#a-draft-explanation-by-vlam)): `{ "concept", "model" }`. Records nothing. Without Vlam configured the route does not exist |
| `POST /api/backoffice/testomgeving/leegmaken` | admin | In test mode, empty the test setup (see [Modes](#modes)); in stable mode the route does not exist |
| `GET /api/backoffice/beheer/partijen` | admin | Per external party the submissions that wait for its decision (`open`, each as `stuk` with `na` as in `werkvoorraad`), its decisions not sent yet (`toe_te_zenden`) and the executor's requests it has not answered (`verzoeken`); under `antwoord_op_verzoek` the facts that answer a request; under `gekoppeld` the parties with a server of their own ([the link](/components/regelhet-koppeling#entering-for-a-linked-party-after-all)) |
| `POST /api/backoffice/beheer/vastleggingen` | admin | A fact an external party records, with the body of `vastleggingen`; an answer to an open request names its receipt under `verzoek` |
| `POST /api/backoffice/beheer/voorvertoning` | admin | What an external party's decision would come to; records nothing |
| `POST /api/backoffice/beheer/besluiten` | admin | An external party's decision on a submission it holds |
| `POST /api/backoffice/beheer/toezendingen` | admin | A copy of an external party's decision to the cell it concerns |

A citizen's application gets `bsn_aanvrager` from the login, and every other field the UI spec has the system fill from the server ([below](#what-the-server-fills-in-an-application)); a request that sends such a field is refused with `422`. A submission that refers to a record (`op`) must refer to one in the citizen's own dossier, and any submission other than the application must refer to one. The dossier is the set of records that hold the citizen's number in that field, with every record that refers to one of them, however indirectly: the decision on the application, an objection to that decision. It is derived from the chronicles on every request and kept nowhere else. Beside the records it carries their standing (`stand`), kept to what is about them and naming no record outside the dossier: an entry that would name one is left out.

An employee names records by id. A record outside the executor's chronicle answers `404`, the same as an id that does not exist; under `/api/backoffice/beheer/` the same holds for a record outside the external parties' chronicles. A cell named in the path or body is public, since the case names its cells: a fact for an external party through `vastleggingen`, or its chronicle, answers `403`, and a cell that is no external party answers `404` in the admin panel. The forms list the fields of a record only for the records the caller may see: a citizen's own dossier, or the executor's chronicle. Each record entered through the admin panel keeps the admin's account in `recorded_by` and `beheerpaneel` in `recorded_via`, while the record itself names the party as its `recording_actor`.

### What answers a submission

Which record is a submission, a decision or a recorded fact, and what answers a submission now, the server takes from the kern of the chronolexography PoC (`stand.rs`), which reads it from the records and the regelwerken. The screens do not work it out themselves. A decision or a fact whose article gives it an effect (`effects`, `revokes`) revokes the record that effect names; a record that is itself revoked revokes nothing. A submission is answered by its last decision while that stands. When a later record revoked it, the answer is the decision that record took in its place, such as the new decision a decision on an objection holds (Awb 7:11). With nothing in its place, a submission whose decision a recorded fact revoked waits for a decision again; one whose decision another decision revoked, such as a withdrawal, waits for nothing.

That last case is how deciding again works. When a court annuls a decision on an objection with its legal effects, the receipt of the judgment revokes it, and the objection is back in the work queue with that receipt as `na`. The decider decides on it after the receipt, and the cell records the new decision with `after`. The admin panel's `open` follows the same rule for an external party. A decision is offered under `toe_te_zenden` only when its article is one whose copy the law requires a receipt of, the rule `kern::toezenden` applies.

The forms carry the UI spec of the source as the PoC applies it (`src/ui_spec.rs` there): which field is prefilled from which output, which the system fills and which the form hides, and the `label` and `help` a screen shows for a field. The screens are on [RegelHet Screens](/components/regelhet-frontend); the server serves the build of its surface from `REGELHET_FRONTEND`. A path under `/api/`, `/auth/` or `/fsc4b/` that is no route answers `404`; any other path gets the screens' `index.html`.

## The timeline of an application

Both sides show, per application, what happened about it and what comes next. One derivation serves both: the kern collects the records (`tijdlijn.rs` in the chronolexography PoC) and the server adds the next steps (`src/timeline.rs`). A record belongs to an application when it links to one that does, by the links records carry themselves: `refers_to`, `after`, `received`, or a field that holds the id of a record, such as the application a request to the GGD was made for. The answer to a request and its receipts name nothing; the kern's pairing of requests links them. The duplicate of a lost card refers to the decision that granted the card, so it lands in the timeline of the first application. The order follows each chronicle and the links between records, then the day. A test clock that stands still puts a whole process on one day, so the day alone would not do.

The next steps come from what the server already knows, never from a step of a particular law:

- a submission that waits for a decision (`stand`), with the cell that decides on it;
- a request of the policy the other party has not answered;
- the next act after a grant, the system's when the case gives it `door_het_systeem`;
- a decision of an external party whose copy was not sent yet;
- a decision that still counts and gives the field a submission form reads as its deadline (`termijn`), until that day, while no such submission was made against it.

Each step names who is to act: the executor, the system, an external party (entered in the admin panel) or whoever may submit. Some things are not there because nothing generic says them. The kern cannot tell which submission a deadline is for, since the objection and the appeal read the same field, so the timeline names the deadline and the label of its field, not the kind of submission. It also gives no deadline for a decision itself, such as the period a court sets for a new one: that is an output of the judgment that no rule marks as a deadline.

A citizen gets the records of their dossier and no step that names another. An employee gets the executor's own records; the court's judgment shows through the receipt of its copy, and an appeal in the court's chronicle does not show at all.

## A UI spec in layers

A manifest names its UI spec under `ui_spec`: one file, as the parking card does (`packages/regelhet/ui_spec.yaml`), or a list of files, the layers of one UI spec. The source holds a list as one file of several YAML documents, in the order of the list (`source_manifest`), and the server reads it as one register (`ui_spec::lees`). A typical list is a generic layer that says what any application and any decision shows under the general administrative law, then the layer of one executor that fills it in, then a layer with the letters:

```yaml
ui_spec:
  - ui/generiek.yaml
  - ui/uitvoering.yaml
  - ui/brieven.yaml
```

Each layer has `elements`. An element has:

| key | what the server does with it |
|---|---|
| `id`, `element` | names it; a letter and a table are called after it |
| `builds_on` | the id of an element of another layer this one fills in. The server merges the two before anything else reads them: the fields and the outcomes by name (the keys of a field here go over the generic ones, a field of its own follows them), every other key this element does not give itself from the generic one (the pattern, the parts, the step). The generic element then drops out; one that nobody fills in stays as it is |
| `role` | who sees it: a role that names whoever applies or objects (`aanvrager`, `belanghebbende`, `burger`, `indiener`) is the citizen's side, and so is the role of an element that fills in an element of such a role (`rol_generiek`), whatever the executor calls it; `beslisser` and `bestuursorgaan` are the decider's; any other role, such as one that examines an application, is for every employee, the reader included (`gedeeld/rollen.ts`) |
| `basis` | left to the register: the provisions the element rests on |
| `fields` | the fields it shows, asks or fills, each by `name` (below) |
| `adds` | parts it adds to the generic element, each with `part`, `place`, `fields` and `facts`. A part that names fields and facts is a table per field: its rows the list of records the submission holds there, each row with the records of those facts it comes from (below) |
| `facts` | facts whose records the element shows: a table per fact, of the records that belong to the submission |
| `outcomes` | left to the register: what follows from what the element records |
| `letter`, `parts` | a letter ([the decision as a letter](#the-decision-as-a-letter)) |
| `table` | a table given in full: `rows` (`field`, `outcome` or `fact`), `columns` (each a `name` and a `label`), `provenance` (each a `fact` and the keys to `match` on), `label` and `help` |

A field has `name` and:

| key | what the server does with it |
|---|---|
| `visibility` | `asked`: an input; `shown`: shown, read-only; `hidden`: not on the form |
| `fill` | `{by: system, ...}`: the server fills it ([below](#what-the-server-fills-in-an-application)). `{from: <regelwerk>#<article>#<output>}` without `by`: a prefill. The form starts with what that output gives for whoever logged in, shows it read-only with where it comes from, and lets the applicant change it with a button; what the applicant submits is what the application holds (`voorgevuld` in `GET /api/formulier`) |
| `editable` | listed per element (`elementen` in `GET /api/formulier`): whether the user may change the field there |
| `required` | `true`: the form does not submit without it |
| `to_build` | listed per element: the model does not carry the name yet |
| `label`, `help`, `group`, `record`, `parts` | how a screen names and asks it |

Every other key of an element or a field is left to the register, so a register that grows a key does not stop the server. `GET /api/formulier` gives every element as the layers merge it (`elementen`: id, element, role, its fields with their visibility, whether they may be changed and how they are filled, and the parts it adds), and the tables (`tabellen`). The back office lists beside a submission the elements for the employee's level, with what the submission and its decision hold of their fields.

## A table from the chronicle

`GET /api/backoffice/tabellen/{id}` gives, beside a submission of the executor, every table of the UI spec: per row its values and where it comes from (`herkomst`): the record that holds it, and per fact of the table the last record of it, in any cell's chronicle, that matches the row. A row matches a record on the keys of `match`, or without `match` on every key the row and the record share whose value is no list or record. A row that no record of a fact matches says so (`zonder_herkomst`). Of a record of another cell the table gives the cell, the fact, the record's id and the day, not what else the record holds. A table of a fact holds the records of that fact that belong to the submission: those that match a row of a list of records the submission holds on the keys they share, or else the submission itself; a fact that shares no key with the submission gives every record. `gedeeld/TabelUitKroniek.vue` shows a table with `nldd-table`, a column per key and a tag per record a row comes from; the application form shows a list of records the system filled as such a table, read-only and without sources. The tables come from `packages/regelhet/src/table.rs`.

## What the server fills in an application

The UI spec decides how the value of each field of the application comes about. A field without `fill` is the citizen's to fill in. For a field with `fill: {by: system}` the server makes the value itself when the application comes in, inside the step that records it, so the value belongs to the same day and the same chronicles as the record (`kern::systeemvelden`):

| UI spec | Value |
|---|---|
| `{by: system}` | the login: the identifier under the subject claim |
| `{by: system, login: naam}` | the login: the name it gave, empty when it gave none. A login of this server keeps no other claim, so the server refuses to start with a UI spec that asks one |
| `{by: system, submission: date}` | the day of the clock (the test clock in test mode) |
| `{by: system, value: ...}` | that fixed value |
| `{by: system, from: <regelwerk>#<article>#<output>, parameters: {...}}` | the outcome of that article, run on what the server knows of the submission |

`parameters` passes that knowledge to the article under the names of its parameters: `date`, the day of submission, and `login_method`, the means the citizen logged in with. The test login gives `DigiD`, or `eHerkenning` for a case that logs in on a KvK number; the login through Keycloak gives `Keycloak`, and FSC voor Burgers gives `FSC voor Burgers`. For the parking card the date of the application is the day of the clock, the requested decision is the fixed value `gehandicaptenparkeerkaart`, and whether the application is signed follows from article 8 of the municipality's fictitious policy: an application submitted after a login with DigiD counts as signed. Through Keycloak or FSC voor Burgers it does not, so such an application is recorded as unsigned. That is a deliberate choice: the policy names one method it trusts, and a channel it does not name gets no signature it never granted. A session of FSC voor Burgers carries no name, so the name stays empty on that channel.

These fields are input fields of the application: Awb 4:2 asks that an application holds them. The rule generates a value for such a field, and the execution of the law reads it as it would read the same value typed in, so nothing changes in how the law is executed. That is why they are no open terms that policy fills. The choice of the executor sits in an article of its own policy that can be read, explained and changed apart from the law.

This holds for every submission a citizen makes, not only the case's own application: the server fills, per submission, the fields its form asks that the UI spec has the system fill. `GET /api/formulier` gives a citizen, per field the system fills, the value the server would record now (`waarde`), so the form shows what will be recorded for the fields it does not hide. For a hidden field the forms still say how the system fills it, so a screen that shows a recorded application can name the origin. When a rule cannot run, the form says why under `systeem_fout`, and the submission refuses with the reason.

## What the system fills in a decision

A decision asks the authority its findings. Some of them do not differ per application: the provision a decision is taken under, on whose behalf a mandated employee signs, the way it is made known. The UI spec may have the system fill such a finding, with a fixed value or the day (`fill: {by: system, value: ...}` or `{by: system, submission: date}`), as it fills a field of an application. The forms give them per decision under `systeem`, with the value the server would record today, and those the UI spec hides under `verborgen`; the back office does not ask them and lists the shown ones under the form. The server fills them in the step of the decision and of its preview (`kern::besluit_systeemvelden`), and refuses a request that names one. A finding that depends on the application stays the decider's. Only the executor's own decisions get them: what an external party decides, it decides itself.

A field of an application that the system fills from a source (`from`) reads the application as the system fills it, the outcomes of the other sources included: a source that needs another's outcome, such as an amount over a designation the system took from a register, runs once that one has run. A field the applicant leaves empty and the law lets be absent reaches the sources as `null`.

## Messages to the citizen

A message to the citizen follows from a record in the chronicle: a decision, a submission or a fact. Which message follows from which record, and how it is made from that record, says the message spec beside the UI spec (`berichten.yaml`, named under `berichten` in the manifest); the text comes from the text layer of the authority (`berichtteksten.yaml`, under `berichtteksten`), a register with a template per message whose paragraphs carry keys (`{aanvraagdag}`) and conditions (`als: toekenning`). The spec holds no sentence: it binds every key to a source (an outcome of the record, a field of it or of the submission it answers, an outcome in the chain of the decision, what a submission gives under its own article), so no text stands loose from the data.

Each kind of message (`berichtsoorten`) binds to a record (`vastlegging`: role, article, fact, legal character, the outcome that carries it), follows a pattern of the generic Awb layer whose parts the law requires (`beschikking`, `beslissing_op_bezwaar`, `buitenbehandelingstelling`, `ontvangstbevestiging`, `herstelverzoek`, `mededeling`), and lays out its parts: per part its place (which also gives its order), the article that requires it (`vereist_door`), its source, its condition (`als`) and the NLDD element that shows it. It has a status (concept, laid down, made known) and a channel (e-mail, Awb 2:8; the portal with a notice, 2:10; paper, 3:41), and invariants: the amount in the decision is the total of its specification (`bedrag_is_totaal`), and exactly one judgment holds and fits the outcome (`oordeel_past`). Always checked besides: every part the law requires is filled or shows as missing with its article, and no key is without a source or a value.

The poc reads both files (`berichten.rs`) and refuses what would drop a part without a word: a place no message has, a required part absent, an unknown element, a key or condition of a template without a source. The forms carry them under `berichten`. One component shows every message, `gedeeld/Bericht.vue` with `gedeeld/bericht.ts`: to the citizen beside the record, and to the decider in a sheet under a decision taken ("Toon brief"), with whether each invariant holds. The screen to decide shows the decision, not the letter; see [the back office screens](/components/regelhet-frontend). A violation is never silent. Before the letter goes out, a banner beside "Bericht verzenden" says it breaks its template; the sheet lists each violation, and the citizen sees a notice that the message is not in order. The parking card has four kinds on the template: the decision (grant and refusal), the acknowledgement of the application, the invitation to complete it (Awb 4:5, a fact of article 12 of the fictitious policy) and the decision on objection. The template, and how to add a kind of message, is in `packages/regelhet/BERICHTEN.md`.

A message is shown as the letter of an authority: a band with the sender as text (never a logo; the design system has none for a sender other than the central government), the return address over the address block, a column of references on the right (date, our reference, your letter of, subject, channel), the text, the signature, an annex on a sheet of its own and a footer; printed, a letter on paper of 210 by 297 mm with "Blad x van y" on each sheet, and a stamp on a concept. The text layer gives the head and foot (`briefhoofd`). An annex in columns (`bron.bijlage`) takes its rows from the model, a key whose value is a list of rows or rows the spec builds from keys, and every heading and cell from the text layer (`bijlagen`); it does not compute: the amount per row is the model's, a sum per block is the sum of its rows, and the rows must make the total, which with `bedrag_is_totaal` must be the amount decided.

A message that goes out is a record, not a view built again each time. The spec names the fact of sending (`verzending.feit`): the sender records it in its own chronicle with the kind of message, the record it belongs to, the channel, the day and the message frozen as the citizen reads it, with a SHA-256 hash of it. The citizen reads it from that fact under "Mijn berichten" in her dossier, and the employee sees the version that went; a later change of the spec, the text layer or the record does not change it, and a new message replaces the last one by naming it. The day it went is the day the decision was made known. Forwarding a message (`verzending.doorsturen`) is a fact of the same article as its receipt by another cell, such as the court; the core delivers it as any fact between cells. Of two facts with the same parameters, the one the article lists first is sent and the later one is its receipt, so the case's cell records by hand a fact it forwards and never the receipt of one. For the parking card these facts are article 13 of the fictitious policy.

## After a grant

A grant is not the end of the process. For the parking card the municipality orders the card from the printer, its post room establishes that the printed card came in, and the municipality hands it to the citizen, who collects it or gets it by post. Each is a fact the municipality records in its own chronicle, and each refers to the decision that granted the card.

The facts are in the municipality's fictitious policy on cards it issued (`beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal`), each with what it holds. The order is in article 7, which has the system take it (see [Steps the system takes itself](#steps-the-system-takes-itself)); the other two are in article 5:

```yaml
- fact: kaart_besteld_bij_drukker      # article 7
  parameters: [datum_bestelling]
- fact: gedrukte_kaart_ontvangen       # article 5
  parameters: [datum_ontvangst, ontvangst_vastgesteld_door]
- fact: kaart_uitgereikt
  parameters: [datum_uitreiking, per_post_verzonden]
```

The printer is a supplier, not an authority, and has no cell. The municipality records its own acts and what it observes.

The aggregate per decision of the municipality's reduction (`reductie_geldige_gehandicaptenparkeerkaart`, article `kaart`) plays these facts with the decision they refer to. Its state keeps which record granted the card (`toekenningsbesluit`, from article 6 of the same policy) and the day of each act. That record is the decision on the application, or a decision on objection that put a grant in its place. A grant revoked on objection, or annulled by the court, leaves `toekenningsbesluit` empty, as a refusal does. The case gives that no further consequence: acts recorded before the revocation stay in the chronicle, and the grant leaves the list of grants that wait. What should become of a card that was already handed out is an open point. When a decision on objection puts a new grant in the place of the old one, the acts already done count for the new grant too, and the next act refers to the decision on objection; whether such a card has to be printed again, the case does not say. A card the court grants itself (Awb 8:72, third paragraph, under b) has no act after it here: the municipality does not derive that card.

The case file names the aggregate, the property of the grant, the acts in their order and the text of each (`na_toekenning`):

```yaml
na_toekenning:
  aggregaat: reductie_geldige_gehandicaptenparkeerkaart#kaart
  toekenning: toekenningsbesluit
  stand: Uw kaart is toegekend. De gemeente bestelt haar bij de drukker.
  handelingen:
    - feit: beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#7#kaart_besteld_bij_drukker
      gedaan: datum_bestelling
      door_het_systeem:
        artikel: beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#7
        parameters:
          datum_bestelling: date
          toekenningsbesluit: submission
      stand: Uw kaart wordt gedrukt.
    - feit: beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal#5#gedrukte_kaart_ontvangen
      gedaan: datum_ontvangst
      knop: Ontvangst van de gedrukte kaart vastleggen
      stand: Uw kaart ligt klaar.
    # the handing out follows, with a knop as well
```

An act with `knop` is a person's; an act with `door_het_systeem` is the system's and has no button.

The kern (`packages/poc-chronolexografie/src/vervolg.rs`) refuses such a fact unless it refers to a record that grants and is the next act there: the first one whose day is still empty. So a handing out before the receipt is refused, and so is a second receipt or anything after a refusal. The check runs inside the step, on the chronicles the step commits against, so two employees who record the same act at once cannot both succeed. The server holds no rule of the parking card; a case without `na_toekenning` has no acts after a grant.

The UI spec has the system fill the day of each act with the day of the clock, so the test clock sets those days in test mode, and fills `ontvangst_vastgesteld_door` with `postkamer`. The server fills them in the step, as it fills an application, and refuses a request that names them. A fact holds only the fills the system knows itself, a fixed value or the day: a fact comes from an employee, not from a citizen's login.

## Steps the system takes itself

Some steps need no person: no judgement, no observation and nothing done by hand. The system takes them, in the commit of the step that brings them about, so they land together with it or not at all. There is one mechanism for all of them. The case file names an article of the executor's own policy and what the system passes it, and the kern runs it (`opvolgen` in `verzoek.rs`). The article has one boolean output, which says whether to act, and requires one fact the executor establishes, which the system then records with the values of the article's parameters. The parameter values are `date`, the day of the step, or `submission`, the id of the record that brings the step about. Two places in the case file declare such a step:

| Declared under | Occasioned by | For the parking card |
|---|---|---|
| `bij_aanvraag` | an application | article 9 of the parking-card policy: the request to the GGD |
| `door_het_systeem` of an act in `na_toekenning` | a grant that counts and has that act next | article 7 of the policy on issued cards: ordering the card |

After every step, whatever it was, the server records each system act that is now next for a grant the step changed (`vervolg::automatisch`): a grant on the application, a grant on objection, or a new decision after an annulment. A step about something else brings about no system act, also not one another grant still has pending. A system act passes the same check as a person's ([`vervolg::toets`](#after-a-grant)): its turn, once per grant, and only while the grant counts. Nobody can record it by hand. In the work queue its row says the next step waits for the system and has no button. One system act can make the next act a system act too, so this runs in rounds until a round records nothing. A round advances each grant by at most one act, so there are never more rounds than acts; past that the step fails and records nothing.

The store keeps `beleid` as `recorded_via` and the article as `recorded_by` for each such record. The chronicle and the dossier pass that on under `door_het_systeem`, per record the article, and the screens add "Automatisch vastgelegd volgens …" to the row. A retry of the step under its `Idempotency-Key` gets the first answer, so the system act is not made twice. `POST /api/backoffice/voorvertoning` also returns `automatisch`: per system act that the decision would bring about, the fact, the article and the record it refers to.

Which steps of the parking card are a person's and which the system's:

| Step | By | Why |
|---|---|---|
| Application, objection, appeal | the citizen | a choice of the citizen |
| Request to the GGD, and its receipt there | the system (article 9) | follows from the application by policy |
| The GGD's advice, the court's ruling and its copy to the municipality | the admin, for that party | the admin panel stands in for that party |
| Receipt of the advice or the ruling at the municipality | the system | delivered with the fact or the copy |
| Copy of the contested decision to the court | the system | goes with the appeal |
| A decision, also on objection or after an annulment | a decider | a judgement |
| Making a decision known to the citizen | no step | the decision is in the dossier as soon as it is recorded |
| Ordering the card | the system (article 7) | nothing to judge or observe |
| Receipt of the printed card | a person of the post room | an observation |
| Handing out the card | a decider | done by hand |
| Copy of an own decision to another cell (`toezendingen`) | a decider | kept by hand: which decision goes where is not settled in the case |

## A draft explanation by Vlam

On the back office a decider can ask for a draft explanation of a decision that was taken. Vlam, the language model service of the Rijksoverheid, writes it in plain language. The button "Concepttoelichting laten opstellen" sits under the decision in the work queue. The server calls Vlam's OpenAI-compatible API itself (`POST {base}/chat/completions`), so the key never reaches the browser.

What goes to Vlam is the decision as an employee of the executor sees it: the article, the date, the outcome and the reasons (`waarom`, with its chain of steps). Before it leaves, the server drops every value whose key names a person, such as the citizen service number, a name, an address or a date of birth, and every record id (`without_personal_data` in `src/vlam.rs`). The facts of the decision do go, health findings included, because they are what the explanation is about. The filter goes by key names: a free-text field, or a field a new case names in a way the list does not know (a licence plate, a KvK number), passes. Treat what reaches Vlam as personal data under the AVG, and settle a DPIA and a processing agreement before real data goes through it. The server follows no redirect from Vlam, asks for a draft of at most 800 tokens and reads at most 256 KiB of the answer. The draft is marked on screen as the text of a language model. The decider edits it, copies it or discards it. Nothing is recorded and the decision does not change.

The feature is off unless a key is set. Without one, `GET /api/ik` says `concepttoelichting: false`, the screens hide the button and the route does not exist.

| Variable | Default | Meaning |
|---|---|---|
| `REGELHET_VLAM_API_KEY` | none | The API key. Setting it turns the feature on |
| `REGELHET_VLAM_BASE_URL` | none | The OpenAI-compatible base of Vlam, up to and including `/v1`. Required with the key |
| `REGELHET_VLAM_MODEL` | `vlam-code-vast` | The model |

In the chart, put the key in a Secret in the release's namespace and name it in `vlam.existingSecret` (key `api-key`, or `vlam.secretKey`), with `vlam.baseUrl` beside it. Only the back-office instance gets the variables:

```bash
kubectl create secret generic vlam-fieldlab --from-literal=api-key="$VLAM_KEY"
helm upgrade <release> charts/regelhet --reuse-values \
  --set vlam.existingSecret=vlam-fieldlab --set vlam.baseUrl=<base-url>
```

## A lost card

A citizen whose card was handed out and is lost or stolen reports it and applies for a duplicate in one submission. Article 52 of the BABW is the ground: the authority that issued the card gives a duplicate for a card that is lost or destroyed (first paragraph), handed out only against a declaration signed by the applicant that the card is lost, which describes the circumstances (third paragraph). Article 53, first paragraph, under b, says a card loses its validity when a duplicate is issued. Those texts are in the corpus; the machine-readable form of article 52 is in the case corpus of the PoC (`packages/poc-chronolexografie/corpus/regulation/nl/amvb/besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw`). The report is the declaration, so one submission covers both: that follows from the third paragraph, and the citizen fills in one form.

```yaml
# BABW article 52, as the case models it
produces:
  - type: AANVRAAG
    requests: besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#52
    parameters: [bsn_aanvrager, dagtekening, aanvraag_ondertekend, verloren_kaart,
                 datum_vermissing, omstandigheden_verlies, verklaring_naar_waarheid]
  - type: BESCHIKKING
    decision_type: TOEKENNING
    outputs: [duplicaat_verstrekt]
```

The submission refers to the record that granted the card but takes nothing over from it: only a submission that holds the decision it refers to as a field, such as an objection, does that (`Cell::record_submission`). The UI spec has the system fill the citizen number, the day and the signature, the latter by article 8 of the municipality's fictitious policy, as for the application; the name comes from the login as well. The field `verloren_kaart` holds the record the submission refers to, which the system fills (`submission: referred_record`), so the decision knows which card is lost. The citizen fills in the address, which Awb 4:2 asks, and the report. The rest comes from the municipality's fictitious policy:

| What | Where |
|---|---|
| Whether the card reported lost is the card that counts now in its aggregate, handed out and still valid | open term of article 52, bound by article 11 of the parking-card policy to an answer of the municipality's cell (`aanvrager_heeft_uitgereikte_geldige_kaart`, reduction `reductie_geldige_gehandicaptenparkeerkaart`, read by article 8 of the policy on issued cards) |
| What a duplicate does with the card | article 9 of the policy on issued cards: the acts after a grant start again at the decision on the duplicate, and the last day of validity stays |

The decision grants a duplicate when the declaration is signed and says the card is lost, the circumstances are given, and the card it names counts, was handed out and is still valid. In the aggregate of the card, the decision on the duplicate becomes the record that granted, so the lost card no longer counts there and the duplicate is ordered by the system, received by the post room and handed out, as any card. No request to the GGD follows: the case's `bij_aanvraag` runs for the application for a card only.

The case file offers the report beside a card whose acts are all done (`na_toekenning.aanvragen`, with the text of the way in and of the form), and gives the line a citizen reads under a card a new one took the place of (`vervallen`). A second report about the same card, while the first waits or later, is refused once the first granted a duplicate: the card it names no longer counts. A report about a card not yet handed out, about a refusal, or about an expired card is refused. A lost duplicate can be reported in turn, once it is handed out. A card found again after a duplicate is an open point: the case gives it no consequence, and the old card stays invalid.

## The post room

To record the receipt, an employee of the post room needs `regelhet-beslisser`, the role that also takes decisions: no step of the ladder records facts without deciding. The record says the post room established it, and `recorded_by` names the account. A role per department in Keycloak, so that only the post room records a receipt and decides nothing, is an open point.

## Running it locally

```bash
just regelhet
```

This starts the server in test mode on port 7180, bound to `0.0.0.0`, with the chronicles in memory and the test login. It serves the screens once `npm run build -w frontend-regelhet` has built them. Set `REGELHET_PORT` for another port and `REGELHET_MANIFEST` for another manifest. To keep the chronicles across restarts, point it at a PostgreSQL database:

```bash
REGELHET_STORE=postgres DATABASE_URL=postgres://postgres:postgres@localhost:5432/regelrecht just regelhet
```

This serves both sides on one address. To run the two environments apart, start an instance per surface on the same database, each on its own port and in a terminal of its own:

```bash
# terminal 1, the citizens
REGELHET_STORE=postgres DATABASE_URL=postgres://postgres:postgres@localhost:5432/regelrecht \
  REGELHET_SURFACE=burger REGELHET_PORT=7182 just regelhet
# terminal 2, the back office
REGELHET_STORE=postgres DATABASE_URL=postgres://postgres:postgres@localhost:5432/regelrecht \
  REGELHET_SURFACE=backoffice REGELHET_PORT=7183 just regelhet
```

## Checking a case before it runs

A case that does not load stops the server at start, and on the cluster that shows only after the pod has restarted a few times. `regelrecht-regelhet check <manifest>` gives the same verdict in a second, without a server, a database or a login:

```bash
just regelhet-check packages/regelhet/iit-groningen/bron.yaml
```

```
case:       individuele_inkomenstoeslag (Individuele inkomenstoeslag)
executor:   gemeente_groningen (gemeente Groningen)
article:    participatiewet#36
outcome:    individuele_inkomenstoeslag_verleend
cells:      gemeente_groningen, rechtbank_noord_nederland
start date: 2026-10-13
ok: packages/regelhet/iit-groningen/bron.yaml starts
```

The check builds the server as it starts in test mode, with the store in memory and the test login, and drops it again (`src/check.rs`). It therefore fails on what the start fails on: a manifest that does not read, a case file the source does not accept, a UI spec the server refuses. It reads no environment, so a `REGELHET_STORE` or `OIDC_*` on the machine does not change the verdict. What it cannot see is what depends on the deployment: the database, the Keycloak of stable mode, a linked party.

A manifest that makes several cases needs the name of the one to check as a second argument, `regelrecht-regelhet check <manifest> <case>`: what `REGELHET_CASUS` is to the server. The check sees what a start in test mode refuses, and not what only stable mode refuses, such as a login the case needs.

Exit code 0 means the case starts, 1 that it does not, with the reason on stderr, and 2 a call without a manifest. A manifest repository can run the check in its own CI with the image, on a runner with Docker:

```bash
docker run --rm -v "$PWD:/case:ro" code.digitaledienst.overheid.nl/regelrecht/regelhet:<commit> \
  regelrecht-regelhet check /case/bron.yaml
```

Take for `<commit>` the version the case runs on, so the check uses the server that will run it. An image from before the check existed ignores the argument and stops with `REGELHET_MODE is not set`.

## Deployment

CI builds the image `regelrecht-regelhet` (`packages/regelhet/Dockerfile`) when the crate or what it depends on changes. The image holds the builds of the screens under `/app/frontend`, so one image runs as either surface, and as the front of an [organisation](/components/regelhet-organisatie). Neither the preview nor the production deploy rolls it out yet: the `regelrecht` deployment has no RegelHet component. See [Deployment](/operations/deployment).

Until then the server runs as a test setup in a ZAD deployment of its own, `fieldlab`, apart from the previews and with a database of its own:

| Component | Host | `REGELHET_SURFACE` | Login |
|---|---|---|---|
| `regelhet-burgers` | `regelhet-burgers-fieldlab-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl` | `burger` | the test login, with the simulated DigiD |
| `regelhet-medewerkers` | `regelhet-medewerkers-fieldlab-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl` | `backoffice` | behind the access wall, then Keycloak with the client `regel-k4c-fieldlab` (`REGELHET_LOGIN=oidc`) |

Both run in test mode on PostgreSQL. A push to the pull request of the integration branch builds the image; nothing rolls it out on its own. Putting a build into `fieldlab` takes `zad deployment update-image fieldlab --component <component> --image ghcr.io/nederlandsedigitaledienst/regelrecht-regelhet:sha-<tag>` for each component, and `zad logs fieldlab --component <component>` shows the startup line of the new container.

On a Fundament cluster with Flux, such as one with Haven+, the plugin in `packages/fundament-plugin` lets project members create RegelHet instances from the console: each is a `RegelHet` object in a namespace Fundament made, and runs one case from a git repository. The plugin copies the case into the namespace once, with a Job, at install and after a change of the manifest, and the pods unpack that copy at every start. They need no token and no network access to git, and the plugin can delete the token after the copy. Like `fieldlab` on ZAD, each runs an instance per surface on a PostgreSQL database of its own, here a CloudNativePG `Cluster`. In test mode, the default, both surfaces use the test login. In stable mode (`spec.auth.mode: stable`, a choice in the console's wizard) they log in through a Keycloak of the release itself, on a third address `<name>-login`, with a client per surface, a database of its own and the realm imported at its first start. The plugin installs the Helm chart `charts/regelhet` through a Flux `HelmRelease` per RegelHet, from `main` or, with a `version`, from that commit. The plugin has [its own page](/components/fundament-plugin); the resource, the addresses and how the hosts are reached are in `packages/fundament-plugin/README.md`.

Rolling it out takes two components in the `regelrecht` deployment, from the same image and on the deployment's database (`DATABASE_SERVER_FULL`):

| Component | Domain | `REGELHET_SURFACE` | Keycloak client |
|---|---|---|---|
| `regelhet` | `regelhet.regelrecht.rijks.app` | `burger` | `regelhet` |
| `regelhet-backoffice` | `regelhet-backoffice.regelrecht.rijks.app` | `backoffice` | `regelhet-backoffice` |

1. The Keycloak roles, the two clients and their mappers of [RegelHet roles](/auth-and-roles#regelhet-roles).
2. Each component added by hand, with `zad component add <component> --image ghcr.io/nederlandsedigitaledienst/regelrecht-regelhet:<tag> --deployment regelrecht --port 8000 --service publish-on-web`, its domain, and as user env vars:

   | Variable | `regelhet` | `regelhet-backoffice` |
   |---|---|---|
   | `REGELHET_MODE` | `stable` | `stable` |
   | `REGELHET_SURFACE` | `burger` | `backoffice` |
   | `OIDC_CLIENT_ID` | `regelhet` | `regelhet-backoffice` |
   | `OIDC_CLIENT_SECRET` | the secret of that client | the secret of that client |
   | `OIDC_DISCOVERY_URL` | the realm's discovery URL | the same |
   | `OIDC_REQUIRED_ROLE` | `regelhet-toegang` | `regelhet-toegang` |
   | `BASE_URL` | `https://regelhet.regelrecht.rijks.app` | `https://regelhet-backoffice.regelrecht.rijks.app` |

   FSC voor Burgers, when it is configured, goes on `regelhet` only; its variables are on [FSC voor Burgers](/components/regelhet-fsc4b).
3. Changes in `deploy.yml`:
   - `build-regelhet` in the `needs` and the conditions of `deploy-preview` and `deploy-production`;
   - its result in the check that refuses a half deploy;
   - `add RegelHet regelrecht-regelhet` and `add regelhet-backoffice regelrecht-regelhet` in both component lists.

Both components migrate the schema `wetsproces` and make the sessions table at startup, under one advisory lock, so they take turns. Previews clone their components from `regelrecht`, so a preview can only include the server once the components exist there. A preview gets its own hosts, so their redirect URIs must be allowed on the two Keycloak clients as well, and its `BASE_URL`s differ from production's.
