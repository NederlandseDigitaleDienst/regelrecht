---
title: "Authentication & Roles"
description: "The layered role model that gates the editor and harvester-admin services, and how it maps to Keycloak."
---

This page describes the role model that gates the editor and harvester-admin
services, how the roles are wired up in Keycloak, and the per-deployment
configuration each service needs.

## The four-layer model

Access is governed by a layered set of realm roles. The hierarchy is encoded
in Keycloak as **composite roles**, a higher role contains the lower one, so
a user holding a higher role automatically holds all lower roles in their
token. The application code never has to check `role-A OR role-B`; each
protected route asks for exactly one role.

```
                  regelrecht-admin
                 /              \
        editor-admin            harvester-admin
              │                       │
        editor-writer           harvester-writer
              │                       │
        editor-reader           harvester-reader
```

A line means "the role above contains the role below" (configured as a
Keycloak composite role).

There are two applications today:

- **editor**: `packages/editor-api` + `frontend/` (law and scenario editor)
- **harvester**: `packages/admin`, a standalone harvester job-queue & corpus API. Its dashboard UI is served inside the editor as the "Corpusinwinning" section (`frontend/src/harvester/`), visible to any `harvester-*` role and reached through the editor-api `/api/harvest-admin/*` proxy (which forwards the session cookie so the harvester API enforces the `harvester-*` gates below). The API stays independently addressable for non-editor clients.

### Role reference

| Role | Grants |
|---|---|
| `editor-reader` | Editor: read user-scoped data (favorites, settings), keep your own Corpus juris favorites, and harvest search. |
| `editor-writer` | Editor: edit laws & scenarios inside a traject (see [Traject membership](#traject-membership)), manage settings and traject favorites, enqueue harvests. Inherits `editor-reader`. |
| `editor-admin` | Editor: corpus reload, feature-flag changes. Inherits `editor-writer`. |
| `harvester-reader` | Harvester admin: read jobs, sources, law entries, platform info. |
| `harvester-writer` | Harvester admin: enqueue harvest and enrich jobs. Inherits `harvester-reader`. |
| `harvester-admin` | Harvester admin: delete jobs, reset exhausted entries, sync sources. Inherits `harvester-writer`. |
| `regelrecht-admin` | Everything across both apps. Inherits `editor-admin` and `harvester-admin`. |

## Specific (orthogonal) rights

Some functions need their own gate, separate from the writer/reader ladder.
Example: not every `editor-writer` should be able to publish a law. Model
these as their own realm role (e.g. `editor-publish`) and add them to the
relevant `<app>-admin` composite. Users who need the right without the full
admin role get it granted explicitly on top of their writer role.

```
                 editor-admin
                /      |       \
       editor-writer   editor-publish   (and any others)
              |
       editor-reader
```

To add a new specific right:

1. Create a new realm role `<app>-<verb>` in Keycloak.
2. Add it to the composite `<app>-admin` (so admin keeps inheriting everything).
3. Optionally assign it to users who need it without the admin role.
4. In the application code: `route_layer(require_role("<app>-<verb>"))` on the
   protected route.

No changes are needed to existing routes; the pattern is composable.

## Traject membership

Realm roles decide which tier of routes a user reaches. Within the editor, a
second layer decides which trajects (shared editing sessions) they can see and
change. Corpus edits have no route of their own outside a traject: they go
through `/api/trajects/{id}/corpus/...`, so every write names its traject in
the URL.

Each member of a traject has one of two roles, stored per traject in the
editor database rather than in Keycloak:

| Traject role | Grants |
|---|---|
| `owner` | Everything a contributor can do, plus renaming or deleting the traject, adding, changing and removing members, and withdrawing invites. |
| `contributor` | Read the traject and edit the laws, scenarios and notes on its branch. |

The realm role is still the outer gate: reading a traject needs
`editor-reader`, changing anything in it needs `editor-writer`. The handlers
then look up the caller's membership on every request and answer `403` to a
non-member. An `editor-reader` who has been invited can therefore read a
traject's in-progress edits but not change them.

## JWT shape

The application reads `realm_access.roles` from the ID token. With composite
roles, the token contains the *effective* set of roles after expansion:

```json
// regelrecht-admin user
"realm_access": {
  "roles": [
    "regelrecht-admin",
    "editor-admin", "editor-writer", "editor-reader",
    "harvester-admin", "harvester-writer", "harvester-reader"
  ]
}

// editor-writer user (no publish right)
"realm_access": {
  "roles": ["editor-writer", "editor-reader"]
}
```

## Keycloak setup

1. **Create the roles** in the realm: all seven plus any specific rights you
   need.
2. **Configure composite roles**:
   - `editor-writer` → contains `editor-reader`.
   - `editor-admin` → contains `editor-writer` (+ every editor specific right).
   - `harvester-writer` → contains `harvester-reader`.
   - `harvester-admin` → contains `harvester-writer` (+ every harvester
     specific right).
   - `regelrecht-admin` → contains `editor-admin` and `harvester-admin`.
3. **Realm-roles mapper on the ID token**: each OIDC client (editor,
   harvester-admin) needs a "Realm roles" mapper that injects `realm_access`
   into the **ID token** (not just the access token). Keycloak only adds it
   to the access token by default; without this mapper the service falls
   back to parsing the access token, which is noisier in the logs.
4. **Assign roles to users**.

## Per-deployment configuration

Each service is gated on a **minimum role** at login time, configured via
`OIDC_REQUIRED_ROLE`. Per-route checks layer finer-grained roles on top.

| Component | `OIDC_REQUIRED_ROLE` |
|---|---|
| `editor` | `editor-reader` |
| `harvester-admin` | `harvester-reader` |

If `OIDC_REQUIRED_ROLE` is unset or empty, the service falls back to
`allowed-user`, a realm role from before the per-app roles existed, and logs a
warning on startup. Always set the value explicitly, so the login gate matches
the per-app reader role (`editor-reader` / `harvester-reader`).

### Setting the env var on ZAD

The `zad-actions/deploy@v4` GitHub action used in `.github/workflows/deploy.yml`
takes only `image` and `clone-from`; env vars are set out-of-band per
component via the ZAD CLI or dashboard. For preview deploys, `clone-from:
regelrecht` carries the value from the production deployment automatically,
you only need to set it once per environment.

```bash
# Production
zad component edit editor --deployment regelrecht \
    --env OIDC_REQUIRED_ROLE=editor-reader
zad component edit harvester-admin --deployment regelrecht \
    --env OIDC_REQUIRED_ROLE=harvester-reader
```

## Operational notes

### Role-change propagation

The role set is read from the JWT at login and cached in the session
(`SESSION_KEY_ROLES`) for the lifetime of that session. Per-request middleware
reads this cached list rather than re-parsing the token, which means:

- **Role changes in Keycloak only take effect on the next login.** Granting a
  user a new role (e.g. promoting `editor-writer` to `editor-admin`) requires
  the user to log out and back in before the new role is honoured by the
  application.
- **Role revocation has the same delay.** Removing a role in Keycloak does
  *not* immediately revoke access; the live session continues to carry the
  expanded role list until it expires.

For emergency revocation (compromised account, immediate downgrade) the
session store must also be cleared so the cached role list cannot be reused.
Sessions live in the PostgreSQL `tower_sessions.session` table on each
service's database:

```sql
-- Revoke all live sessions for a specific user (by Keycloak sub):
-- The session stores the Keycloak subject under the key `person_sub`
-- (see SESSION_KEY_SUB in packages/auth/src/handlers.rs).
DELETE FROM tower_sessions.session
WHERE data::jsonb -> 'data' ->> 'person_sub' = '<keycloak-sub>';

-- Nuclear option — invalidate every active session on the service:
TRUNCATE tower_sessions.session;
```

After deleting the session row(s), the affected user is forced through the
OIDC login again, which re-reads roles from Keycloak.

A session that carries no role list at all (the `person_roles` key is absent,
as opposed to present and empty) gets a 401 on role-gated routes. That
triggers the OIDC re-login redirect, and the callback writes the role list
from the JWT, so such a session repairs itself without a flush.

### Auth-disabled mode (dev/local only)

When the OIDC environment variables are not configured (`OIDC_CLIENT_ID`
unset), each service starts with **all per-route auth checks bypassed**.
Every tier (reader, writer, *and* admin) is reachable without a session.
This mode exists for local development convenience (no Keycloak required)
and emits a `warn!` line at startup:

```
OIDC authentication is DISABLED — editor is unprotected.
All routes (editor-reader/writer/admin tiers) bypass auth checks.
Do NOT run this configuration in production.
```

The same applies to the harvester-admin service. **Never deploy a service
without OIDC configured**: the warning is the only safeguard, and the
admin-tier routes (corpus reload, feature-flag toggles, job deletion, source
sync) are fully open in this mode.

### Local SSO login (dev/local, real Keycloak)

To test the *real* login + RBAC flow locally, instead of the auth-disabled
bypass above, you can point the editor at a Keycloak realm and log in with a
real account over `http://localhost`. No code change is required; the auth
layer is driven entirely by environment variables.

**How the local flow fits together**

- The editor frontend runs on Vite (started with `--port 7300`) and proxies
  `/api`, `/auth`, `/health` to the editor-api on `:8000`. The browser talks
  only to `http://localhost:7300`. (Vite's config default is `:3000`; we use a
  host-mapped port, see *Ports and DB host* below.)
- The redirect URI is built as `${BASE_URL}/auth/callback`. With
  `BASE_URL=http://localhost:7300` it becomes
  `http://localhost:7300/auth/callback`, which Vite proxies to the backend.
- When OIDC is enabled the session uses a **PostgreSQL** store, so
  `DATABASE_URL` is required (the `just dev` Postgres works; from a dev
  container reach it via `host.docker.internal`).

**Keycloak configuration (one-time, realm admin)**

Configure the OIDC client the editor will use. Re-using the production client
and adding a `localhost` redirect to it is a minor token-leakage footgun, so the
cleaner choice is a single dedicated dev client, `regelrecht-local`, shared
across all local apps (editor, harvester-admin, …), with a `localhost` redirect
URI per app port:

1. **Client type / capability**: *Client authentication* = **On** (a
   confidential client: the editor sends a client secret, and startup fails if
   `OIDC_CLIENT_SECRET` is empty). Enable *Standard flow* (Authorization Code).
   PKCE (S256) is sent by the app and accepted by Keycloak out of the box; you
   may optionally enforce it under *Advanced → Proof Key for Code Exchange*.
2. **Valid redirect URIs**: add exactly `http://localhost:7300/auth/callback`
   for the editor. (The harvester-admin service is API-only; its UI is the
   editor's Corpusinwinning section, reached through the editor-api proxy, so it needs
   no separate browser redirect URI in local dev.) Scheme + host + port + path
   must match the value the app sends.
3. **Web origins**: add `http://localhost:7300`, or `+` to derive from the
   redirect URIs, so the browser's CORS checks pass.
4. **Client scopes**: ensure `openid`, `email`, and `profile` are granted; the
   app requests these three scopes.
5. **Realm-roles ID-token mapper**: add a *User Realm Role* mapper on the client
   (or its dedicated scope) with *Add to ID token* = On. Keycloak only puts
   `realm_access.roles` in the access token by default; without this mapper the
   app falls back to parsing the access token (noisier, but still works).
6. **Roles on your account**: grant your user at least `editor-reader` (the
   login gate set by `OIDC_REQUIRED_ROLE`). Without it you can authenticate but
   get **403** on every API call. See the role hierarchy above for higher tiers.
7. Note the **discovery URL**
   (`https://<host>/realms/<realm>/.well-known/openid-configuration`), the
   **client ID**, and the **client secret** (Credentials tab) for the env file.

**Run it**

Copy `.env.sso-local.example` to `.env.sso-local`, fill in the Keycloak
values, then run `just dev editor`. It does the three steps below in one go;
run them by hand when you need to see each process in its own shell:

```bash
# 1. Postgres only:
docker compose -f docker-compose.dev.yml -f dev/compose.native.yaml up -d postgres
# 2. editor-api on :8000 with .env.sso-local loaded:
just editor-sso
# 3. in another shell — editor frontend on the host-mapped port 7300:
cd frontend && npx vite --port 7300
```

Open `http://localhost:7300` in any browser and log in. When `BASE_URL` is an
http localhost origin the editor drops the session cookie's `Secure` flag, so
Safari (which, unlike Chrome and Firefox, refuses Secure cookies over
`http://localhost`) completes the OIDC handshake too. Outside localhost
(production always serves over an `https://` `BASE_URL`) the cookie stays
`Secure`.

On a successful start the editor-api log shows
`using OIDC_DISCOVERY_URL for issuer: …` and
`session store ready (PostgreSQL-backed)` (not the "OIDC is DISABLED" warning).

**Ports and DB host.** The values above assume a dev container backed by Docker
Desktop (the common setup here):

- `DATABASE_URL` uses `host.docker.internal`: published container ports live on
  the Docker host, not the container's `localhost` (the same reason
  `TESTCONTAINERS_HOST_OVERRIDE` is needed for tests).
- The frontend uses a **host-mapped port** in the 7100–7300 range (`7300`)
  because only mapped ports are reachable from the browser; `3000`/`8000` are
  not forwarded. The editor-api stays on `:8000` (container-internal; only Vite
  needs to be reachable).

On a plain Linux host neither applies: use `localhost` for the DB and any free
port (e.g. `3000`) for `BASE_URL` and Vite, with a matching redirect URI in
Keycloak.

## Programmatic access (admin API key)

The harvester-admin service accepts a bearer API key on **GET**, **POST** and
**DELETE** requests (`ADMIN_API_KEY` env var). This is an out-of-band trust path,
the holder is treated as a `regelrecht-admin`-equivalent on those methods, so
scripts and services can enqueue harvest/enrich jobs (`POST /api/harvest-jobs`,
`POST /api/enrich-jobs`) without driving an interactive OIDC/SSO session. Because
the trust is method-based (not route-based), the key also reaches the admin-tier
POST routes (`reset-exhausted`, source `sync`). That is consistent with it already
permitting the destructive `DELETE /api/jobs`. A user session with the matching
role still works too. The editor service has no API key path.

## Implementation pointers

- Shared crate: `packages/auth/`, `require_role(role)` middleware factory.
- Editor routes: `packages/editor-api/src/main.rs`, router split into
  public / reader / writer / admin groups, plus separate reader and writer
  groups for trajects, personal notes (`/api/user/notes/...`) and review
  tasks (`/api/tasks/...`). Those three also run `account_middleware`,
  because their handlers need the account record.
- Traject membership: `require_membership` and `require_owner` in
  `packages/editor-api/src/trajects.rs`.
- Harvester-admin routes: `packages/admin/src/main.rs`, router split into
  reader / writer / admin groups; `require_auth(role)` in
  `packages/admin/src/middleware.rs` keeps the API-key bypass.
- Roles are persisted in the session at login (`SESSION_KEY_ROLES`) so
  per-request checks don't re-parse the JWT.
