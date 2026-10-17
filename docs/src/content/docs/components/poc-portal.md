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
| `pagina.rs` | The index and the password screen, rendered as HTML with `nldd-*` components |

### The register

`pocs/registry.yaml` is the single place where the PoCs are described. The cards, the served and proxied paths, the password variable per PoC and the deploy filters in `script/deploy-filters.mjs` all derive from it. It is embedded with `include_str!`, so the binary cannot disagree with the register it was built with. To add a PoC or change its status, use the `poc-add` skill, which also covers the deploy wiring the register does not contain.

Each entry has a public half and an internal half. `titel` and `samenvatting` appear on the index and on the password screen, both of which anyone can reach, the latter by guessing a slug. `titel_intern`, `samenvatting_intern`, `tags` and the status are shown only behind the password.

The portal refuses a register with an invalid or duplicate slug, a `voorbehoud` shorter than 30 characters, a static PoC without a `bron`, a proxied PoC without an `upstream`, or a proxied PoC with the assistant switched on. `status` has no default: it is one of `verkenning`, `in-ontwikkeling` or `gevalideerd`, and there is deliberately no value for production.

### Two kinds of PoC

- **`statisch`**: a Vue app built into this image with `POC_BASE=/<slug>/` and served from `/app/static/<slug>` with its own single-page fallback. Today these are `frontend-poc-terugbetaalregimes/` and `frontend-poc-nieuwkomersbekostiging/`, each with its case corpus under `corpus-poc/`. They run the engine as WASM in the browser.
- **`proxy`**: a PoC that runs as its own component, not published on the web, and is forwarded to in-cluster at `http://{deployment}-{upstream}:8000`. The deployment name comes from the pod's hostname, so a preview reaches its own upstream and not production's. Outside the cluster, `POC_UPSTREAM_<SLUG>` sets the address. The path is forwarded unchanged and cookies pass both ways, because the proxied app keeps its own session.

The first proxied PoC is napp, in `packages/poc-napp/` with its frontend in `frontend-poc-napp/`. It runs the subsidy process from a bill as a chain of requests, decisions, payment orders and objections, on an Axum backend with SQLite and a reconstruction of the bill as its law. It has its own README.

The second is vakantieverlof, in `packages/poc-vakantieverlof/` with its frontend in `frontend-poc-vakantieverlof/`. It plays through an application for leave under article 13a of the Leerplichtwet 1969 at a fictional school, using the cell from `packages/cel`: the application, the school head's decision, the objection and the decision on it, the chronicle and the school's reduction.

Each decision shows why it came out as it did, as the cell explains it (`Cell::explain`): who filled each open term, and the engine's trace under the law in force. The head is never asked whether the pupil already had leave in the school year. The cell answers that from its chronicle through its reduction. In the trace that answer is a single step, which names the reduction version and the last chronolexogram the cell read. Asked how it got there, the cell runs the reduction again and shows its own trace (`Cell::explain_answer`). That trace shows the decisions revoked on objection and how many decisions the filter skipped.

Its chronicle lives in memory, shared by every visitor, and is empty after a restart. Its regelwerken come from `corpus/regulation/`, not from `corpus-poc/`.

### The gate

One shared password per PoC, read from `POC_PW_<SLUG>` (the slug in capitals, hyphens as underscores). The portal will not start when a PoC in the register has no password, since that PoC would otherwise be open to anyone.

After a correct password the portal sets an HMAC-signed cookie that names the slug and an expiry. The signing key is `POC_COOKIE_SECRET`, separate from the passwords so that rotating one password leaves the other PoCs' cookies valid. The cookie only says "this browser knew the password"; a PoC's own login, such as napp's, sits behind it unchanged. The redirect after signing in stays under the PoC's own prefix, and the content security policy is `POC_CSP` from `packages/auth/src/security_headers.rs`. The details are in `gate.rs`.

### The policy assistant

A static PoC can have `assistent: true`. The image then also carries `packages/poc-assistent/`, a Node server that runs the Claude Code CLI headless with MCP tools specific to one case, and `start.sh` starts one process per case on localhost, from port 3600 upward, and exports `POC_ASSISTENT_<SLUG>` so the portal knows where to forward `/<slug>/api/...`. Without a `CLAUDE_CODE_OAUTH_TOKEN` or `ANTHROPIC_API_KEY` no assistant starts, `/api` answers `503`, and the app hides its assistant panel.

The list of cases comes from `POC_ASSISTENT_CASUSSEN` in `start.sh`, not from the register; its default is `terugbetaalregimes nieuwkomersbekostiging`. A new PoC with `assistent: true` therefore also needs that default or the variable changed.

### Image and deployment

`packages/poc-portal/Dockerfile` builds the engine to WASM, the portal's design-system bundle from `frontend-poc-portal/`, each static PoC with its own base path, and the Rust binary. The portal, napp and vakantieverlof deploy as three ZAD components, `poc`, `napp` and `vakantieverlof`; see [Deployment](/operations/deployment). Only `poc` is published on the web. Because the image builds the engine, `script/deploy-filters.mjs` ties the `poc` component to two crates, the portal binary and the engine, so an engine change rebuilds the image even though the portal itself does not depend on the engine.

## Running locally

```bash
just poc
```

This builds the WASM engine, the design-system bundle and the static PoCs into `.poc-static/`, then starts the portal at `http://localhost:8611`. The recipe sets the local cookie key and the password `demo` for every PoC; both are dev-only values written in the Justfile, and production reads its own from ZAD.

`just poc-build`, the step behind it, names the static PoCs one by one (`terugbetaalregimes` and `nieuwkomersbekostiging`), as does `packages/poc-portal/Dockerfile`. Neither is derived from the register, so a new static PoC has to be added to both by hand.

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

### vakantieverlof

The image sets the port to `8000`, the base path to `/vakantieverlof/`, the corpus to `/app/corpus` and the built frontend to `/app/static`. `just poc-vakantieverlof` runs it locally.

| Variable | Default | Purpose |
|----------|---------|---------|
| `VERLOF_PORT` | `8410` | Listening port |
| `VERLOF_BASE_PATH` | `/` | Path prefix; must match the portal path |
| `VERLOF_STATIC_DIR` | `dist` under `frontend-poc-vakantieverlof/` in the repository | Built frontend, made by `npm run build` |
| `VERLOF_CORPUS_DIR` | `corpus/` in the repository | The regelwerken under `regulation/nl` and the cell under `cells/obs_de_klimop` |

## Further reading

- [Shared Frontend Package](./frontend-shared) - the saved-state, variant and runner modules the static PoCs use
- [Demo](./demo) - the one public entry on the index
- [Deployment](/operations/deployment) - the `poc`, `napp` and `vakantieverlof` components
