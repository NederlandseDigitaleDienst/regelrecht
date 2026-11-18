---
title: "Wetsproces"
description: "A server that runs one legal process on the cells of chronolexography for many users at once, with its chronicles in a store and the date from a clock."
---

The wetsproces component is a wetsprocessysteem: a server that runs one legal process, from application to decision and on to objection and appeal, for everyone who uses it. It is one implementation built on the regelrecht method, not the way regelrecht itself has to be run. The process steps are those of the chronolexography PoC; this component adds what a shared server needs on top of them.

## Overview

- **Language**: Rust (Axum)
- **Location**: `packages/wetsproces/` (crate `regelrecht-wetsproces`)
- **Intended URL**: `wetsproces.regelrecht.rijks.app` (not deployed yet)
- **Port**: 8000
- **Process**: the disabled parking card of the fictitious municipality of Wilgendaal, from `packages/wetsproces/bron.yaml`

## What it runs

The server loads exactly one source manifest at startup, the same `bron.yaml` format the [PoC portal](/components/poc-portal) serves sources from, read by the same code (`packages/corpus/src/source_manifest.rs`). The manifest ships with the image; it is never fetched from GitHub. It must make exactly one case, and the server refuses to start when it makes none, several, or one that does not load.

The first case is the disabled parking card (BABW, article 49). It runs three cells: the municipality that decides, the GGD that examines the applicant and advises, and the administrative court that decides on an appeal. The regelwerken and cells come from the PoC's corpus; only the case file in `packages/wetsproces/casussen/` belongs to this component.

## How a request runs

Where the PoC keeps one visitor's chronicle in the browser, this server keeps every cell's chronicle in a chronicle store. Each request loads the cells of the case from the store, runs the step of the PoC's kern (`packages/poc-chronolexografie/src/kern.rs`, used as a library), and writes what the step recorded back in one commit. That commit covers every cell the step read. A step can record in more than one cell: the GGD's advice lands in the GGD's chronicle and in the municipality's.

The store (`ChronicleStore` in `src/store.rs`) loads a chronicle per cell and appends to several cells at once, all or nothing. Each cell in a commit names the head it was read at, the number of records in it then. When another request appended in between, the commit is refused and the server runs the step again on the new state, three times at most. A cell the step only read is part of the commit as well, so a decision taken on an outdated view of the GGD's chronicle is refused too.

A writing request may carry an `Idempotency-Key` header. Sent again with the same key and the same request, it gets the first answer and nothing is recorded twice. The same key with a different request is refused with `409`.

What day it is comes from a clock (`Clock` in `src/clock.rs`). Every step runs on that day and the cell records it as the day of the record.

## Modes

`WETSPROCES_MODE` selects the mode and has no default:

- `test` keeps the chronicles in memory, empty at every start, and uses a clock the API sets with `PUT /api/klok`. It starts on the case's `speeldatum`.
- `stable` is meant to keep the chronicles in PostgreSQL, in a schema `wetsproces` of the shared database, and to use the real date in the Netherlands (`Europe/Amsterdam`). Neither the store nor a login exists yet, so the server refuses to start in this mode with a message that says so, rather than fall back to memory.

The server has no login yet. Anyone who reaches it can submit and decide, which is why only test mode starts. Login through Keycloak comes in a later change, before stable mode can run.

## API

The keys in requests and answers are those of the kern, as the PoC uses them (`velden`, `bevindingen`, `op`). An error answers `{ "fout": "<reason>" }`.

| Method and path | What it does |
|---|---|
| `GET /health` | `ok` once the server runs |
| `GET /api/casus` | The case and today's date |
| `GET /api/formulier` | The forms of the process today, with what the UI spec says of each field |
| `GET /api/cellen/{cel}/kroniek` | The chronicle of one cell, with its head |
| `POST /api/indieningen` | A submission: `artikel` (the case's own when left out), `op`, `velden` |
| `POST /api/besluiten` | A decision on a submission: `op`, `na`, `bevindingen` |
| `POST /api/vastleggingen` | A fact a cell records: `cel`, `feit`, `velden`, `verwijst_naar` |
| `POST /api/toezendingen` | A copy of the decision `besluit` to the cell it concerns |
| `GET /api/klok`, `PUT /api/klok` | Today's date; in test mode, set it with `{ "datum": "YYYY-MM-DD" }` |

The forms carry the UI spec of the source as the PoC applies it (`src/ui_spec.rs` there): which field is prefilled from which output, which the system fills and which the form hides. There is no frontend yet.

## Running it locally

```bash
just wetsproces
```

This starts the server in test mode on port 7180, bound to `0.0.0.0`. Set `WETSPROCES_PORT` for another port and `WETSPROCES_MANIFEST` for another manifest.

## Deployment

CI builds the image `regelrecht-wetsproces` (`packages/wetsproces/Dockerfile`) when the crate or what it depends on changes. Neither the preview nor the production deploy rolls it out yet: the `regelrecht` deployment has no `wetsproces` component, and without a login the server should not be reachable in production. See [Deployment](/operations/deployment).

Rolling it out takes, once login is in place:

1. The component `wetsproces` in the `regelrecht` deployment, added by hand (`zad component add wetsproces --image ghcr.io/minbzk/regelrecht-wetsproces:<tag> --deployment regelrecht --port 8000 --service publish-on-web`), with the domain `wetsproces.regelrecht.rijks.app` and `WETSPROCES_MODE` set on the component.
2. Then, in `deploy.yml`, `build-wetsproces` in the `needs` and the conditions of `deploy-preview` and `deploy-production`, its result in the half-deploy check, and `add wetsproces regelrecht-wetsproces` in both component lists. Previews clone their components from `regelrecht`, so they can only follow once the component exists there.
