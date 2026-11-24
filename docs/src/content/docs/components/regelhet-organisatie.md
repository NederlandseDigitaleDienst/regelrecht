---
title: "RegelHet: the organisation"
description: "How the RegelHet server, in the role organisatie, puts one login, a page of tiles and one work queue in front of several processes of one organisation, and hands each person over to a process with a signed statement."
---

A municipality runs more than one legal process: the disabled parking card, a parking permit for businesses, later more. Each runs as a [RegelHet](/components/regelhet) of its own, with its own addresses, login and database. The organisation is a front before them: one address for citizens and one for employees, one login, a page with a tile per process, and for employees one work queue over all processes. A tile or a row of the queue opens the process without a second login. The processes keep their own containers, databases and screens, and each still works on its own.

The front is the same server from the same image, started with `REGELHET_ROL=organisatie`. It runs no process and keeps no chronicle. The code is in `packages/regelhet/src/organisatie/`, the screens in `frontend-regelhet/src/organisatie/`.

## The signed statement

The front speaks for whoever logged in at it with a statement it signs: a compact JWS with ES256, from the same JOSE code [FSC voor Burgers](/components/regelhet-fsc4b) uses, with the RFC 7638 thumbprint of the front's key as `kid`.

| Claim | What |
|---|---|
| `iss` | the organisation's id, `<namespace>/<name>` |
| `aud` | `<process id>#burger` or `<process id>#backoffice` |
| `sub`, `rollen`, `claims`, `naam` | the account of the front's session; of its roles only those of this server (`regelhet-...`) that the side admits, so the realm's own roles such as `offline_access` stay at the front |
| `inlog` | `testlogin` or `keycloak`, how the front logged the account in |
| `modus` | the front's mode, `test` or `stable` |
| `iat`, `exp` | `exp` at most 60 seconds after `iat` |
| `jti` | unique per statement |

A process takes a statement only when it is signed with its organisation's key, `iss` is its organisation, `aud` names this process and a side the instance serves, the time is within `iat` and `exp` (30 seconds of clock difference allowed), and every role starts with `regelhet-`. A process in stable mode refuses a statement with `modus: test`. Each `jti` is taken once; the process remembers it in memory until it expires, so with more than one replica of a process a statement could be taken once per replica within its minute.

The process then filters the roles again: first by the side the statement is for, then by its own surface. An employee's role in a statement for `#burger` stays behind, and so does the citizen's role in one for `#backoffice`. One role ladder holds for the whole organisation, so a `regelhet-beslisser` at the front is a decider in every process behind it. Roles per process are not built.

The front signs a statement in two places:

- **A tile.** `GET /api/processen/{id}/openen?naar=<path>` answers `303` to the process's public address, `/auth/organisatie?token=<statement>&naar=<path>`. The process checks the statement, writes a session as its login does (what the session held before is dropped), and sends the browser to `naar`, which must be a path on the process. A citizen's tile leads to the application form (`/burger#nieuwe-aanvraag`), an employee's to the work queue (`/backoffice`).
- **A call.** The front calls a process inside the cluster with the statement in the header `Authorization: Organisatie <statement>`. The process takes such a statement only when the request has no session, and only for a `GET`; anything else answers `403`.

What a person records in a session the front handed over is kept with the channel `organisatie` in `recorded_via`, and with `<organisation id>:<sub>` as the account in `recorded_by` (migration `0011`). An application states how the front logged the person in, as it would after the process's own login.

## One work queue

An employee lands on the work queue of the organisation (`/backoffice` on the front); the tiles are its second tab, `Regelingen`. The screen asks every process at once, through the front, for its case, its forms, its queue and the requests of its policy (`casus`, `formulier`, `backoffice/werkvoorraad` and `backoffice/verzoeken`), and for the executor's chronicle only when a decision was revoked and the row has to say after what. The requests and the chronicle only add words to a row, such as whom it waits on; when one of them fails, the rows of that process stay. Each row is made by the same code as a process's own work queue (`frontend-regelhet/src/gedeeld/werkvoorraad.ts`) and carries the name of its process. The rows of all processes form one list, the longest waiting first; a row says whom it waits on when a request about it is open, as in the process itself. Grants whose next act waits are not in this list, only the submissions that wait for a decision. A segmented control above it narrows the list to one process.

A row opens the submission in the handling screen of its process, `/backoffice/behandelen/<cel>/<id>`, through the hand-off. A process that does not answer, or answers with an error, gets a notice above the list with the reason, and the rows of the other processes stay. The asking itself is one function, `perProces` in `frontend-regelhet/src/organisatie/perProces.ts`, which gives per process its answer or why there was none.

## A citizen's applications

The citizens' side of the front has two tabs: **Aanvragen**, the page of tiles, and **Mijn aanvragen**, the citizen's applications in every process of the organisation in one list, newest first. The front has no route of its own for that list. The screen asks each process for `casus` and `dossier` through the passed-on routes and makes the rows with the code a process's own list uses (`frontend-regelhet/src/gedeeld/dossier.ts`). A row therefore says what the process's own screen says of the same application: what was applied for, on which day, and how it stands. A tag names the process. The row opens the application at `/burger/dossier/<id>` in its process, through `openen`, so the citizen arrives there logged in.

A process that does not answer drops out of the list, and a notice above it names that process; the applications in the others stay. A process that answers `403` on `dossier` knows citizens by another identifier than the one the login carries. The statement passes every claim of the front's login on, and each process reads its own claim (`REGELHET_SUBJECT_CLAIM`, `bsn` unless set). A process that reads `kvk_nummer` finds nothing to go on in a DigiD login, and the list leaves it out without a notice, because none of its applications can be this citizen's.

## The front's routes

| Method and path | Who | What |
|---|---|---|
| `GET /health`, `/auth/...` | anyone | the login, as on a process |
| `GET /api/uiterlijk` | anyone | the accent colour, and `organisatie: { naam, logo, voorkant: true }` |
| `GET /api/ik` | citizen or employee | who is logged in, in the shape of a process's, without cells |
| `GET /api/organisatie` | citizen or employee | the name, the logo, the side (`kant`), and per process its `id`, `naam`, the `titel` of its case and whether it answered (`bereikbaar`) |
| `GET /api/processen/{id}/api/{*path}` | citizen or employee | the process's own answer, for the routes below only |
| `GET /api/processen/{id}/openen?naar=<path>` | citizen or employee | `303` to the process, as above |
| `GET /organisatie/v1/sleutel` | anyone | the public key of the statements, a JWK |

`{id}` is the process id from the file of processes, URL-encoded. A path with a backslash, `?`, `#`, `%` or a control character passes nowhere, so the process receives the path that was checked. The front passes on, for the back office, `casus`, `ik`, `formulier`, `backoffice/werkvoorraad`, `backoffice/verzoeken`, `backoffice/vervolg` and `backoffice/cellen/{cel}/kroniek`; for citizens, `casus`, `ik`, `dossier` and `formulier`. Any other path answers `404`, as an unknown process does. A session at the front of someone who is both a citizen and an employee, which only happens on `all`, reaches the processes as a citizen. A process that does not answer within five seconds gives `502` for that process alone, and the tile page shows it as not reachable while the other tiles keep working.

The test login of the front offers the test accounts of its processes, asked from each process's `/auth/testpersonen` and merged: each account once, the citizens deduplicated by their number.

## Configuration

The front, on each of its two instances:

| Variable | Meaning |
|---|---|
| `REGELHET_ROL` | `organisatie`; without it the server is a process (`proces`) |
| `REGELHET_SURFACE` | `burger` or `backoffice`, or `all` on one machine |
| `REGELHET_MODE` | `test`; stable mode is refused for now |
| `REGELHET_LOGIN`, `REGELHET_ACCENT_COLOR`, the OIDC variables | as on a process |
| `REGELHET_ORGANISATIE_ID` | `<namespace>/<name>` of the organisation, the `iss` of its statements |
| `REGELHET_ORGANISATIE_NAAM` | the name it shows, such as `Gemeente Wilgendaal` |
| `REGELHET_ORGANISATIE_LOGO` | an `https` URL of its logo; optional |
| `REGELHET_PROCESSEN` | the file of processes below |
| `REGELHET_ORGANISATIE_SLEUTEL` | the file with the private key, PKCS#8 PEM on P-256 |

Both instances sign with the same key, so the key is required. Only on `all`, where one instance serves both sides, the front makes a key at start when the variable is missing. In this role the server ignores `REGELHET_MANIFEST` and loads no case. It refuses `REGELHET_STORE=postgres`, FSC voor Burgers, the variables of a [link between servers](/components/regelhet-koppeling) (`REGELHET_LINK_*`) and `REGELHET_PROCES_ID`. It serves the build `organisatie` of the screens under `REGELHET_FRONTEND`, on either surface.

The file of processes, which the Fundament plugin (`packages/fundament-plugin`) writes into the values of the front's release:

```yaml
processen:
  - id: tnt-voorbeeld--parkeerkaart/parkeerkaart
    naam: parkeerkaart
    burger:
      intern: http://parkeerkaart-burger.tnt-voorbeeld--parkeerkaart.svc.cluster.local:8000
      publiek: https://parkeerkaart.cluster-a.example.org
    backoffice:
      intern: http://parkeerkaart-backoffice.tnt-voorbeeld--parkeerkaart.svc.cluster.local:8000
      publiek: https://parkeerkaart-medewerkers.cluster-a.example.org
```

`intern` is where the front calls the process, `publiek` where it sends a browser. An id appears once, and every address is an `http(s)` URL.

A process that belongs to an organisation, beside its own variables:

| Variable | Meaning |
|---|---|
| `REGELHET_PROCES_ID` | `<namespace>/<name>` of the process, the `aud` it takes; setting it makes the process a member |
| `REGELHET_ORGANISATIE_ID` | the `iss` it takes |
| `REGELHET_ORGANISATIE_NAAM` | the organisation's name in its screens; the id when not set |
| `REGELHET_ORGANISATIE_URL` | the front on the side this instance serves, for the link back; optional |
| `REGELHET_ORGANISATIE_SLEUTEL_URL` | where to fetch the front's key, `<front>/organisatie/v1/sleutel`; test mode only |
| `REGELHET_ORGANISATIE_SLEUTEL_JWK` | the front's key itself, a JWK |

One of the two key variables is required. A process fetches the key at its first statement, and fetches it again at most every ten seconds when a statement names another key or the last fetch failed. Without `REGELHET_PROCES_ID`, any of the other variables stops the server, and so does a fetched key in stable mode. A member process has the route `GET /auth/organisatie` and gives `organisatie: { naam, url }` on `GET /api/uiterlijk`; its screens put the name with a link back in the bar. Without membership neither exists, and the process runs as it did.

## Running it locally

`just regelhet-organisatie` starts two processes in test mode, the parking card on port 7184 and the IIT of Groningen on port 7185, and a front on `all` on port 7186 before them. The processes fetch the front's key from it. Open `http://localhost:7186`, log in as the decider, and the work queue shows what waits in both processes; a row opens it in its process without a second login. The tiles lead to `127.0.0.1`, because a browser keeps cookies per host and not per port: on one host the process's session would replace the front's.

## What is not built

Stable mode for the front, with a Keycloak of its own, the key pinned in each process, and its sessions in PostgreSQL, follows later. A restart of the front ends its sessions, and a new process in the file restarts it. The front reaches the processes over the cluster network without mutual TLS; in test mode someone inside the cluster could offer another key at a process's key URL, which adds nothing to what the open test login already allows. The hand-off is a link a browser follows, so someone could fetch a hand-off for their own account and get another person's browser to follow it within its minute; that browser is then logged in at the process as the first person. Binding the hand-off to the browser that asked for it is not built.
