---
title: "Wetsproces"
description: "A server that runs one legal process on the cells of chronolexography for many users at once, behind a login through Keycloak, with its chronicles in memory or in PostgreSQL."
---

The wetsproces component is a wetsprocessysteem: a server that runs one legal process, from application to decision and on to objection and appeal, for everyone who uses it. It is one implementation built on the regelrecht method; another system can run the same cells in its own way. The process steps are those of the chronolexography PoC; this component adds what a shared server needs on top of them.

## Overview

- **Language**: Rust (Axum)
- **Location**: `packages/wetsproces/` (crate `regelrecht-wetsproces`)
- **Intended URL**: `wetsproces.regelrecht.rijks.app` (not deployed yet)
- **Port**: 8000
- **Process**: the disabled parking card of the fictitious municipality of Wilgendaal, from `packages/wetsproces/bron.yaml`

## What it runs

The server loads exactly one source manifest at startup, the same `bron.yaml` format the [PoC portal](/components/poc-portal) serves sources from, read by the same code (`packages/corpus/src/source_manifest.rs`). The manifest ships with the image; it is never fetched from GitHub. It must make exactly one case, and the server refuses to start when it makes none, several, or one that does not load.

The first case is the disabled parking card (BABW, article 49). It runs three cells: the municipality that decides, the GGD that examines the applicant and advises, and the administrative court that decides on an appeal. The regelwerken and cells come from the PoC's corpus; only the case file in `packages/wetsproces/casussen/` and the UI spec `packages/wetsproces/ui_spec.yaml` belong to this component. The server takes the case's own login field (`inloggen.veld`) as the field that identifies a citizen, or else the one field its UI spec has the login fill (`fill: {by: system}` without a source). Here that is `bsn_aanvrager`, marked in the UI spec. The server refuses to start when it finds no such field or more than one.

## How a request runs

Where the PoC keeps one visitor's chronicle in the browser, this server keeps every cell's chronicle in a chronicle store. Each step loads all cells of the case from the store at one moment, runs the step of the PoC's kern (`packages/poc-chronolexografie/src/kern.rs`, used as a library), and writes what the step recorded back in one commit. A step can record in more than one cell: the GGD's advice lands in the GGD's chronicle and in the municipality's.

The store (`ChronicleStore` in `src/store.rs`) loads a chronicle per cell and appends to several cells at once, all or nothing. Each cell in a commit names the head it was read at, the number of records in it then. When another request appended in between, the commit is refused and the server runs the step again on the new state. After three runs in all it gives up and answers `409`. A cell the step only read is part of the commit as well, so a decision taken on an outdated view of the GGD's chronicle is refused too.

A writing request may carry an `Idempotency-Key` header. Sent again by the same account with the same key and the same request, it gets the first answer and nothing is recorded twice. The same key with a different request, or from another account, is refused with `409`. Without the header, every request records anew and the store keeps no answer for it.

What day it is comes from a clock (`Clock` in `src/clock.rs`). Every step runs on that day and the cell records it as the day of the record.

## Storage

Two stores implement `ChronicleStore`. The store in memory starts empty and is gone when the server stops. The PostgreSQL store (`src/store/postgres.rs`) keeps the chronicles in a schema of its own, `wetsproces`, in the database it is given. Its migrations are in `packages/wetsproces/migrations/`, and sqlx's migration table sits in the same schema. They are independent of the pipeline's migrations and touch no table in `public`.

| Table | Holds |
|---|---|
| `grams` | One row per record: the cell, its sequence number in that cell (from 1), the record's id (unique across all cells), a format version, the record as `jsonb`, when it was stored and `recorded_by`, the Keycloak `sub` of the account whose request recorded it |
| `heads` | Per cell its head |
| `receipts` | Per idempotency key the request, with the account that sent it, and the answer |
| `sessions` | The login sessions, made by the session store (`tower-sessions`) at startup |

`recorded_by` sits beside the record, not inside it: the record says what the law made happen, the column says who asked for it. It is empty for what the server records itself, such as the case's beginning, and for records stored before the login existed (migration `0002_recorded_by.sql`).

`grams` is append-only, and the database enforces it: a trigger refuses every `UPDATE`, `DELETE` and `TRUNCATE` on it. A request that holds the character U+0000 is refused with `422` in both stores, because `jsonb` cannot store it. A record written in a format version this build does not know is not read; the request fails with `500`, and the server log names the cell, the record and the version.

A commit is one transaction. It claims its idempotency key first, so a retry under the same key waits for the first request and then gets its answer. Then it locks the head rows of its cells in order of name, so that two commits never wait on each other in a circle, and checks every head before it appends. Both stores pass one shared set of contract tests (`src/store/contract.rs`), which includes concurrent commits on one cell.

## Modes

`WETSPROCES_MODE` selects the mode and has no default, not even in the image. Without it the server does not start. The mode sets the clock; where the chronicles live is chosen apart from it, with `WETSPROCES_STORE`.

- `test` uses a clock an admin sets with `PUT /api/backoffice/klok`, starting on the case's `speeldatum`. The store is `memory` unless `WETSPROCES_STORE=postgres` asks for PostgreSQL, which gives a lasting chronicle with a clock that can still be moved. With the OIDC variables set it logs in through Keycloak; without them it offers the test login.
- `stable` uses the real date in the Netherlands, in the Europe/Amsterdam time zone, always the PostgreSQL store, and always the login through Keycloak. It refuses to start without the OIDC variables, `OIDC_REQUIRED_ROLE` or `BASE_URL`, and with `WETSPROCES_STORE=memory`. In either mode, an OIDC variable set without `OIDC_CLIENT_ID` stops the server, so a half-finished configuration never opens the test login.

| Variable | Default | Meaning |
|---|---|---|
| `WETSPROCES_MODE` | none | `test` or `stable`. Required. |
| `WETSPROCES_STORE` | `memory` in test mode, `postgres` in stable mode | Where the chronicles live |
| `DATABASE_URL` | none | The database for the `postgres` store; `DATABASE_SERVER_FULL` is read as fallback. Required with that store. |
| `WETSPROCES_MANIFEST` | `packages/wetsproces/bron.yaml` | The source manifest |
| `WETSPROCES_PORT` | 8000 | The port |
| `WETSPROCES_FRONTEND` | `frontend-wetsproces/dist` of the checkout; `/app/frontend` in the image | The built screens. Without an `index.html` there the server serves its API alone |
| `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `OIDC_DISCOVERY_URL` | none | The Keycloak client; `KEYCLOAK_BASE_URL` with `KEYCLOAK_REALM` may replace the discovery URL. Required in stable mode. |
| `OIDC_REQUIRED_ROLE` | none | The role every account needs to log in, `wetsproces-toegang`. Required with OIDC; a role without the `wetsproces-` prefix is refused, because the realm is shared with the editor. |
| `BASE_URL` | none | Where the server is reached, for the redirect back after login. Required in stable mode. |
| `WETSPROCES_SUBJECT_CLAIM` | `bsn` | The claim that identifies a citizen |

When the PostgreSQL store is chosen and the database cannot be reached or migrated, the server stops with that error. It does not fall back to memory.

## Login

Every route except `/health` and `/auth/...` needs a session. With OIDC configured, `/auth/login` logs in through Keycloak with the shared `packages/auth` crate, as the editor does. The roles and their Keycloak setup are on [Authentication & Roles](/auth-and-roles#wetsproces-roles).

Sessions live with the chronicles: in memory with the store in memory, in `wetsproces.sessions` with PostgreSQL. The cookie is `wetsproces_session`, so it never mixes with the editor's sessions.

In test mode without OIDC, `POST /auth/testlogin` gives a session with the roles and claims in its body, without Keycloak:

```bash
curl -c jar -H 'content-type: application/json' localhost:7180/auth/testlogin \
  -d '{"sub": "burger-1", "rollen": ["wetsproces-burger"], "claims": {"bsn": "999990019"}}'
```

The session it writes has the same keys as one after Keycloak, so every request after it goes through the same checks. The route exists only in that case; with OIDC configured, and always in stable mode, it answers `404`. So does `GET /auth/testpersonen`, which lists the accounts the login screen offers, derived from the cells of the case.

## API

The keys in requests and answers are those of the kern, as the PoC uses them (`velden`, `bevindingen`, `op`). An error answers `{ "fout": "<reason>" }`. For a `500` the reason is a fixed sentence and the cause goes to the server log only. A body that does not parse gets axum's own plain-text answer. Without a session the answer is `401`; with a session that lacks the role, `403`.

| Method and path | Who | What it does |
|---|---|---|
| `GET /health` | anyone | `ok` once the server runs |
| `GET /api/casus` | citizen or employee | The case and today's date |
| `GET /api/formulier` | citizen or employee | The forms of the process today, with what the UI spec says of each field |
| `GET /api/klok` | citizen or employee | Today's date, and whether it can be set |
| `GET /api/ik` | citizen or employee | Who is logged in, and per cell what the account may do there |
| `GET /api/dossier` | citizen | The records about the citizen, from every cell |
| `GET /api/dossier/waarom/{id}` | citizen | Why a decision in the citizen's dossier came out as it did |
| `POST /api/indieningen` | citizen | A submission: `artikel` (the case's own when left out), `op`, `velden` |
| `GET /api/backoffice/cellen/{cel}/kroniek` | reader of the cell | The chronicle of one cell, with its head |
| `GET /api/backoffice/waarom/{id}` | reader of the cell holding the decision | Why the decision came out as it did |
| `POST /api/backoffice/voorvertoning` | decider of the cell holding the submission | What a decision (`op`, `na`, `bevindingen`) would come to; records nothing |
| `POST /api/backoffice/vastleggingen` | handler of the cell | A fact a cell records: `cel`, `feit`, `velden`, `verwijst_naar` |
| `POST /api/backoffice/toezendingen` | handler of the cell holding the decision | A copy of the decision `besluit` to the cell it concerns |
| `POST /api/backoffice/besluiten` | decider of the cell holding the submission | A decision on a submission: `op`, `na`, `bevindingen` |
| `PUT /api/backoffice/klok` | admin | In test mode, set the date with `{ "datum": "YYYY-MM-DD" }` |

A citizen's application gets `bsn_aanvrager` from the login; a request that sends it is refused with `422`. A submission that refers to a record (`op`) must refer to one in the citizen's own dossier, and any submission other than the application must refer to one. The dossier is the set of records that hold the citizen's number in that field, with every record that refers to one of them, however indirectly: the decision on the application, an objection to that decision. It is derived from the chronicles on every request and kept nowhere else.

An employee names records by id. A record in a cell whose chronicle they may not read answers `404`, the same as an id that does not exist. The forms list the fields of a record only for the records the caller may see: their own dossier, or the cells they may read.

The forms carry the UI spec of the source as the PoC applies it (`src/ui_spec.rs` there): which field is prefilled from which output, which the system fills and which the form hides, and the `label` and `help` a screen shows for a field. The screens are on [Wetsproces Screens](/components/wetsproces-frontend); the server serves them from `WETSPROCES_FRONTEND`. A path under `/api/` or `/auth/` that is no route answers `404`; any other path gets the screens' `index.html`.

## Running it locally

```bash
just wetsproces
```

This starts the server in test mode on port 7180, bound to `0.0.0.0`, with the chronicles in memory and the test login. It serves the screens once `npm run build -w frontend-wetsproces` has built them. Set `WETSPROCES_PORT` for another port and `WETSPROCES_MANIFEST` for another manifest. To keep the chronicles across restarts, point it at a PostgreSQL database:

```bash
WETSPROCES_STORE=postgres DATABASE_URL=postgres://postgres:postgres@localhost:5432/regelrecht just wetsproces
```

## Deployment

CI builds the image `regelrecht-wetsproces` (`packages/wetsproces/Dockerfile`) when the crate or what it depends on changes. Neither the preview nor the production deploy rolls it out yet: the `regelrecht` deployment has no `wetsproces` component. See [Deployment](/operations/deployment).

Rolling it out takes:

1. The Keycloak roles, client and mappers of [Wetsproces roles](/auth-and-roles#wetsproces-roles).
2. The component `wetsproces` in the `regelrecht` deployment, added by hand with `zad component add wetsproces --image ghcr.io/nederlandsedigitaledienst/regelrecht-wetsproces:<tag> --deployment regelrecht --port 8000 --service publish-on-web`. It needs the domain `wetsproces.regelrecht.rijks.app`, the deployment's database (`DATABASE_SERVER_FULL`) and, as user env vars on the component, `WETSPROCES_MODE=stable`, `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `OIDC_DISCOVERY_URL`, `OIDC_REQUIRED_ROLE=wetsproces-toegang` and `BASE_URL`.
3. Changes in `deploy.yml`:
   - `build-wetsproces` in the `needs` and the conditions of `deploy-preview` and `deploy-production`;
   - its result in the check that refuses a half deploy;
   - `add wetsproces regelrecht-wetsproces` in both component lists.

Previews clone their components from `regelrecht`, so a preview can only include the server once the component exists there. A preview gets its own host, so its redirect URI must be allowed on the Keycloak client as well, and its `BASE_URL` differs from production's.
