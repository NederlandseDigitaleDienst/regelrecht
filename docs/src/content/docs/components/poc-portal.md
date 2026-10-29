---
title: "PoC Portal"
description: "The Rust service that serves several proof-of-concepts behind one hostname, each behind its own password, and marks every page inside a PoC with how far its model of the law has been checked."
---

The PoC portal is the service behind `poc.regelrecht.rijks.app`. It puts several proof-of-concepts under one hostname, one path prefix each, and asks for a separate password per PoC. The PoCs are case studies built with policy teams; they are not the corpus and not law in force.

## Overview

- **Language**: Rust (Axum)
- **Location**: `packages/poc-portal/` (crate `regelrecht-poc-portal`)
- **Production URL**: `poc.regelrecht.rijks.app`
- **Port**: 8000
- **Register**: `pocs/registry.yaml`, compiled into the binary

## What it does

The platform publishes each deployed component on its own subdomain, so three PoCs as three components would get three hostnames and could not share one. The routing between PoCs therefore happens inside a single container: this one.

A request to `/<slug>/...` belongs to the PoC with that slug. Without a valid cookie for that slug, the portal answers `401` with a password form in the body. It does not redirect, so a deep link keeps its address and signing in continues to the page the visitor asked for. After the right password the portal sets a cookie scoped to `/<slug>` and valid for 30 days.

The index at `/` shows every PoC in the register as a card, plus a card for the [Demo](./demo), which is the one entry that needs no password.

Every HTML page served from inside a PoC gets a strip at the top that the portal splices in after `<body>`. It names the PoC, its status and its `voorbehoud` (the caveat, in the PoC's own words), and links back to the index. The strip is there because a computed amount looks equally convincing whether the rules behind it were checked with lawyers or sketched in an afternoon, and a screenshot or a forwarded deep link skips the index where that would otherwise be said.

## Architecture

| Module | Purpose |
|--------|---------|
| `registry.rs` | Parses and validates the register |
| `config.rs` | Reads the environment at startup and refuses to start when anything is missing |
| `gate.rs` | Password check and the signed cookie |
| `app.rs` | The router: index, password form, static PoCs, proxied PoCs, the strip |
| `proxy.rs` | Reverse proxy to a PoC that runs as its own component |
| `bron.rs` | Fetches and caches the sources a static PoC loads while it runs |
| `pagina.rs` | The index and the password screen, rendered as HTML with `nldd-*` components |

### The register

`pocs/registry.yaml` is the single place where the PoCs are described. The cards, the served and proxied paths, the password variable per PoC and the deploy filters in `script/deploy-filters.mjs` all derive from it. It is embedded with `include_str!`, so the binary cannot disagree with the register it was built with. To add a PoC or change its status, use the `poc-add` skill, which also covers the deploy wiring the register does not contain.

Each entry has a public half and an internal half. `titel` and `samenvatting` appear on the index and on the password screen, both of which anyone can reach, the latter by guessing a slug. `titel_intern`, `samenvatting_intern`, `tags` and the status are shown only behind the password.

The portal refuses a register with an invalid or duplicate slug, a source on a proxied PoC or a source name used twice, a `voorbehoud` shorter than 30 characters, a static PoC without a `bron`, a proxied PoC without an `upstream`, or a proxied PoC with the assistant switched on. `status` has no default: it is one of `verkenning`, `in-ontwikkeling` or `gevalideerd`, and there is deliberately no value for production.

### Two kinds of PoC

- **`statisch`**: a Vue app built into this image with `POC_BASE=/<slug>/` and served from `/app/static/<slug>` with its own single-page fallback. A path whose last part has no extension gets the PoC's `index.html` with status 200, so a deep link into the PoC's own router survives a hard refresh; a missing file still answers 404. Today these are `frontend-poc-terugbetaalregimes/`, `frontend-poc-nieuwkomersbekostiging/` and `packages/poc-chronolexografie/frontend/`. The first two take their case corpus from `corpus-poc/`, chronolexografie from `corpus/`. They run the engine as WASM in the browser.
- **`proxy`**: a PoC that runs as its own component, not published on the web, and is forwarded to in-cluster at `http://{deployment}-{upstream}:8000`. The deployment name comes from the pod's hostname, so a preview reaches its own upstream and not production's. Outside the cluster, `POC_UPSTREAM_<SLUG>` sets the address. The path is forwarded unchanged and cookies pass both ways, because the proxied app keeps its own session.

The proxied PoC is napp, in `packages/poc-napp/` with its frontend in `frontend-poc-napp/`. It runs the subsidy process from a bill as a chain of requests, decisions, payment orders and objections, on an Axum backend with SQLite and a reconstruction of the bill as its law. It has its own README.

### chronolexografie

Chronolexografie lives in one directory, `packages/poc-chronolexografie/`: the Rust crate, with its frontend in `frontend/`. A PoC whose core is a Rust crate built to WASM may use this layout; the other PoCs keep `frontend-poc-<slug>/`. The app plays the method of [chronolexography](/concepts/chronolexography) on the cell from `packages/cel`.

The PoC has a corpus of its own, `packages/poc-chronolexografie/corpus/`, with the layout of a source: the cases in `casussen/`, the rulework in `regulation/nl/` and the cells in `cells/`. It holds copies of the laws the cases run, the Awb among them, so work on a case never changes the regelrecht corpus and a change there never breaks a case. The regelrecht corpus keeps its own version of those laws.

Each case is one file in `casussen/`, and this repository holds three. The file names the cell (a directory under `cells/`), the article that establishes the application, and the output of the decision that grants what was asked. The law does not mark that output, so the case file does. The file also lists the case's own corpus under `corpus`, and holds the scenarios with the values each step fills in. The app derives everything else from the law when it loads the case (`src/casus.rs`).

Everything runs in the visitor's browser. The cell, the engine and the app's core (`kern.rs`, `casus.rs`) compile to WebAssembly in this crate, with the feature `wasm`. No server holds state. Each browser tab holds its own game and saves it in `sessionStorage` after every step, so a reload goes on where it was and two tabs never touch each other's game.

Rulework that more than one case runs is shared and sits in `regulation/nl/`. Rulework of one case sits beside its case file, in `casussen/<id>/regulation/nl/`. The vakantieverlof case keeps the Leerplichtwet, the Regeling vaststelling schoolvakanties and the school's policies there. A case runs on the shared rulework its file lists under `corpus`, on the rulework in its own directory, which counts without being listed, and on the reductions of its cells. Other rulework neither runs nor acts on the case, even with a hook on every decision. A case file may not list anything under `casussen/`, and rulework under `casussen/<id>/` needs the case file `<id>.yaml` beside it.

The browser gets the whole corpus in one bundle, `bundel.json`, without the scenarios. The register names the directory under `corpus` for `chronolexografie` in `pocs/registry.yaml`, and `frontend/scripts/copy-assets.js` builds the bundle from it. A test in `src/bundel.rs` fails when the directory holds shared rulework or a cell that no case lists. Because the corpus sits in the crate's directory, `script/deploy-filters.mjs` finds it through the cargo graph and lists nothing for it.

The scenarios run as bucket A over this corpus alone. `just bdd-chrono` loads the shared regulation and that of every case together; `script/chrono-regulation-dirs.sh` lists the directories. CI runs the same in the `bdd-demo` job. `just validate`, the type check and the cross-law check cover the corpus as well.

A fourth case, a fictional subsidy at the Nederlandse autoriteit politieke partijen, lives in the private NAPP corpus. The register lists it under `sources` as `napp`, with the manifest `werkbank/chronolex/napp/bron.yaml` on the branch integratie/analistenportaal. A manifest names the directories of the repository that make up the source, so the NAPP corpus serves as a source without moving anything. The portal serves the source at `/chronolexografie/bron/napp`, behind the PoC's own password (`packages/poc-portal/src/bron.rs`). It fetches the branch from the GitHub API with `NAPP_CORPUS_TOKEN`, a user env var on the ZAD component `poc`, and keeps each fetch for 60 seconds. A push to that branch reaches the PoC within a minute, without a build of regelrecht. The token stays in the portal and never reaches the browser.

A source holds the same directories as the corpus: `casussen/`, `regulation/nl/` and `cells/`. A case from a source runs on that source alone. It brings its own Awb and every other rulework it needs, and nothing of the regelrecht corpus takes their place. A case id that is already taken is set aside, and the app says so.

When GitHub fails, the portal serves its last good copy. Without a copy it answers `502`, and without a token `503`. A source that is not available never stops the app: a banner gives the reason, and the other cases play as before. Errors in the source show only when the page loads, because the CI of regelrecht never sees the source. To work on it locally without a token, set `POC_BRON_NAPP` to a folder or a manifest (see [Running locally](#running-locally)).

### The gate

One shared password per PoC, read from `POC_PW_<SLUG>` (the slug in capitals, hyphens as underscores). The portal will not start when a PoC in the register has no password, since that PoC would otherwise be open to anyone.

The passwords are user env vars on the ZAD component `poc`, set in the ZAD Operations Manager; zad-cli has no command for them. A preview copies the env vars from production when it is created, so a preview made before a password existed fails to start with `NoPassword`.

After a correct password the portal sets an HMAC-signed cookie that names the slug and an expiry. The signing key is `POC_COOKIE_SECRET`, separate from the passwords so that rotating one password leaves the other PoCs' cookies valid. The cookie only says "this browser knew the password"; a PoC's own login, such as napp's, sits behind it unchanged. The redirect after signing in stays under the PoC's own prefix, and the content security policy is `POC_CSP` from `packages/auth/src/security_headers.rs`. The details are in `gate.rs`.

### The policy assistant

A static PoC can have `assistent: true`. The image then also carries `packages/poc-assistent/`, a Node server that runs the Claude Code CLI headless with MCP tools specific to one case, and `start.sh` starts one process per case on localhost, from port 3600 upward, and exports `POC_ASSISTENT_<SLUG>` so the portal knows where to forward `/<slug>/api/...`. Without a `CLAUDE_CODE_OAUTH_TOKEN` or `ANTHROPIC_API_KEY` no assistant starts, `/api` answers `503`, and the app hides its assistant panel.

The list of cases comes from `POC_ASSISTENT_CASUSSEN` in `start.sh`, not from the register; its default is `terugbetaalregimes nieuwkomersbekostiging`. A new PoC with `assistent: true` therefore also needs that default or the variable changed.

### Image and deployment

`packages/poc-portal/Dockerfile` builds the engine and chronolexografie to WASM, the portal's design-system bundle from `frontend-poc-portal/`, each static PoC with its own base path, and the Rust binary. The portal and napp deploy as two ZAD components, `poc` and `napp`; see [Deployment](/operations/deployment). Only `poc` is published on the web. The image builds WASM from two crates the portal does not depend on: the engine and `regelrecht-poc-chronolexografie`. `script/deploy-filters.mjs` therefore ties the `poc` component to those two and to the portal binary, so a change to the engine or the cell rebuilds the image. The corpus of chronolexografie lies in that crate's directory and comes with it.

## Running locally

```bash
just poc
```

This builds the engine and chronolexografie to WASM, the design-system bundle and the static PoCs into `.poc-static/`, then starts the portal at `http://localhost:8611`. The recipe sets the local cookie key and the password `demo` for every PoC; both are dev-only values written in the Justfile, and production reads its own from ZAD.

`just poc-build`, the step behind it, names the static PoCs one by one (`terugbetaalregimes`, `nieuwkomersbekostiging` and `chronolexografie`), as does `packages/poc-portal/Dockerfile`. Neither is derived from the register, so a new static PoC has to be added to both by hand.

`just poc-chronolexografie` builds chronolexografie as WASM and starts its own Vite dev server, without the portal.

To work on a source without a token, point `POC_BRON_<NAME>` at a local folder that has the layout of the source, or at a manifest in a local checkout. A relative path counts from the repository root, where `just` starts:

```bash
POC_BRON_NAPP=<checkout-of-the-source>/werkbank/chronolex/napp/bron.yaml just poc-chronolexografie
```

The Vite dev server then answers the route of the source from that folder (`frontend/scripts/bron-dev.js`), and so does the portal under `just poc`. The folder is read again on every request, so an edit shows after a reload. With the same variable, `cargo test -p regelrecht-poc-chronolexografie` loads every case in the folder and plays its example to a decision. Without it, the NAPP case shows "bron niet beschikbaar" and the test skips the folder.

`just poc` does not start napp or an assistant. The napp card then answers `503` until `POC_UPSTREAM_NAPP` points at a running napp backend. For the assistant, run `just poc-assistent <case>` in a second terminal (it needs `CLAUDE_CODE_OAUTH_TOKEN` or `ANTHROPIC_API_KEY`) and start the portal with the matching variable, for example `POC_ASSISTENT_TERUGBETAALREGIMES=http://127.0.0.1:3600 just poc`.

## Configuration

### Portal

| Variable | Default | Purpose |
|----------|---------|---------|
| `POC_COOKIE_SECRET` | none, required | Key for the cookie signature, at least 32 characters |
| `POC_PW_<SLUG>` | none, required per PoC | The password for that PoC |
| `POC_STATIC_DIR` | `/app/static` | Root of the built static PoCs and of `/_assets` |
| `POC_PORT` | `8000` | Listening port |
| `HOSTNAME` | set by the platform | The pod name; the portal derives the deployment name (`regelrecht`, `pr123`) from it to address proxied PoCs in-cluster |
| `POC_UPSTREAM_<SLUG>` | derived from `HOSTNAME` | Address of a proxied PoC, overriding the in-cluster default; for local development |
| `POC_ASSISTENT_<SLUG>` | set by `start.sh` | Address of the assistant for that PoC |
| `CLAUDE_CODE_OAUTH_TOKEN` | none | Token for the Claude Code CLI the assistant runs on; without it (and without `ANTHROPIC_API_KEY`) no assistant starts |
| `ANTHROPIC_API_KEY` | none | Alternative to the OAuth token |
| `POC_ASSISTENT_CASUSSEN` | both assistant cases | Space-separated list of cases `start.sh` starts an assistant for |
| `NAPP_CORPUS_TOKEN` | none | Token for the `napp` source of chronolexografie: a fine-grained token with only Contents read on the NAPP corpus. The name comes from `token_env` in the register. Without it the NAPP case is unavailable |
| `POC_BRON_<NAME>` | none | A local folder or manifest that replaces the source `<name>`, without a token; for local development |

Passwords and tokens live only in the platform's environment, never in the repository.

### napp

napp runs as its own component and reads its own variables. The defaults below are the binary's; the image sets the port to `8000`, the base path to `/napp/` and the database to `/data/napp.db`.

| Variable | Default | Purpose |
|----------|---------|---------|
| `DATABASE_URL` | `sqlite:napp.db?mode=rwc` | SQLite database |
| `NAPP_PORT` | `8400` | Listening port |
| `NAPP_BASE_PATH` | `/` | Path prefix; must match the portal path, because the portal forwards `/napp/...` unchanged |
| `NAPP_STATIC_DIR` | `dist` under `frontend`, relative to the working directory | Built frontend |
| `NAPP_LAW_DIR` | `law` | Directory with the law YAML napp runs on (`corpus-poc/napp/law/` in the image) |
| `NAPP_MOCK_SSO` | on | Demo login next to real SSO; `0` switches it off |
| `BASE_URL` | none | Public base URL, read by the shared auth crate |
| `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET`, `OIDC_DISCOVERY_URL` | none | SSO Rijk through the shared auth crate; without them only the mock login is available |
| `OIDC_REQUIRED_ROLE` | realm membership | Role required after login; unset, membership of the ZAD realm is enough |

## Further reading

- [Shared Frontend Package](./frontend-shared) - the saved-state, variant and runner modules the static PoCs use
- [Demo](./demo) - the one public entry on the index
- [Deployment](/operations/deployment) - the `poc` and `napp` components
