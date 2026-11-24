---
title: "RegelHet: FSC voor Burgers"
description: "How the RegelHet server offers its citizen routes to an app that proves itself with a Verifiable Credential, after a werkversie of the FSC voor Burgers standard."
---

The [regelhet](/components/regelhet) server can offer its citizen routes as a service after the standard FSC voor Burgers. An app on a phone then proves who the citizen is with a Verifiable Credential and a key it holds, where a browser shows a cookie. This page describes that channel of this one component. It implements a werkversie of the standard, the one at [docs.fsc4b.nl/specs/protocol](https://docs.fsc4b.nl/specs/protocol/) as read in November 2026; section numbers below are that document's. Employees keep logging in through Keycloak, and the browser login for citizens stays as a second channel.

The code is in `packages/regelhet/src/fsc4b/`. Without configuration the channel is off and none of its routes exists.

## What an app does

1. It makes a P-256 key pair, on a phone in its secure hardware.
2. It gets a credential for that key from an Activatiedienst: a JWS with the app's public key in `cnf`, a `scope`, the Activatiedienst's OIN in `iss`, a validity period and the citizen's `bsn` and name (§4.3.1).
3. It reads the Stelseldocument, checks the Stelselbeheerder's signature on it, and takes this organization's public key and discovery URL from it. The discovery configuration names the two services (§4.2).
4. It runs `CreateSession` (§4.3.4). `POST /fsc4b/session/v1/challenge` gives a JWS, signed with the organization's key, with a nonce and an ephemeral P-256 key. The app signs the nonce for this organization's OIN (`aud`), puts that signature, the credential and a fresh AES key in a Verifiable Presentation, encrypts it to the ephemeral key (`ECDH-ES`, `A256GCM`), and sends it with the nonce to `POST /fsc4b/session/v1/complete`. The answer is a session token, encrypted under the app's AES key.
5. It calls the citizen's service with the token in the `Authorization` header and a body that is a JWE under the session key (`alg: dir`, `enc: A256GCM`). The answer comes back encrypted under the same key. Only a `401` (no live session under the token, or a body that does not decrypt under its key) and a `500` while reading the session are plain JSON.

The client in `packages/fsc4b/src/client.rs` does each step as its own function. The end-to-end tests in `packages/regelhet/tests/fsc4b.rs` run on it (the feature `test-utils` re-exports it as `fsc4b::client`), and so does [the app](/components/regelhet-app). The JOSE formats and the names both sides must spell alike are in that crate as well.

## Routes

| Method and path | What it does |
|---|---|
| `POST /fsc4b/session/v1/challenge` | A challenge, valid for one minute and answerable once |
| `POST /fsc4b/session/v1/complete` | The answer to a challenge; gives a session of at most 15 minutes, never past the credential's `exp` |
| `POST /fsc4b/burger/v1/casus` | The case and today's date |
| `POST /fsc4b/burger/v1/formulier` | The forms |
| `POST /fsc4b/burger/v1/dossier` | The citizen's own dossier |
| `POST /fsc4b/burger/v1/waarom` | Why a decision of the citizen's own dossier came out so, for `{ "id": "<decision>" }`, as `GET /api/dossier/waarom/{id}` |
| `POST /fsc4b/burger/v1/indieningen` | A submission, as `POST /api/indieningen` takes it, with its idempotency key as the member `idempotency_key` inside the encrypted body |
| `POST /fsc4b/burger/v1/gegevens` | The source data the executor holds about the citizen, signed by the organization ([below](#source-data-for-the-app)) |
| `GET /fsc4b/regelwerken` | The case as a bundle of files, without a session |
| `GET /fsc4b/discovery` | The discovery configuration: `sessionApi` and `regelhetBurgerApi`, each in version `v1` |
| `GET /fsc4b/openapi.json` | The OpenAPI 3 specification of these routes |

The citizen's routes run the same handlers as the browser routes. A session of this channel is a citizen and nothing else: it holds the role `regelhet-burger` and the credential's BSN under the subject claim, never an employee's role, and it reaches no route outside `/fsc4b/burger/`. The store keeps the channel beside every record, in `recorded_via` (`keycloak`, `testlogin` or `fsc4b`), and the account as `fsc4b:` followed by the RFC 7638 thumbprint of the app's key.

A refused presentation answers `401` with one fixed message, whatever check failed; the server log names the check. The log never holds a nonce, a token or a key. A session is kept under the SHA-256 of its token. Challenges and sessions live with the chronicles, in memory or in the tables `fsc4b_challenges` and `fsc4b_sessions` of the schema `wetsproces`. Taking a challenge deletes its row in the same statement, so two instances of the server never both accept one nonce. About 10,000 challenges wait at once, over all instances; above that `/v1/challenge` answers `503`. The bound is checked without a lock: challenges started at the same moment can each pass the check, so the count can go over by as many as run at once. A limit per client address is left to the ingress in front of the server.

The ephemeral key of a waiting challenge and the AES key of a session are stored as they are, since another instance must be able to use them. Whoever can read the database, or a backup of it, while they live can decrypt the traffic of that challenge or session.

## Source data for the app

An app can predict a decision on the citizen's own device before the citizen applies. It needs two things for that: the case, and what the executor already knows about the citizen. The fields the citizen fills in then stay on the device until they apply.

`GET /fsc4b/regelwerken` gives the case: every file of the source the server runs, as `{ "<path>": "<text>" }` (the case file, the regelwerken, the cells with their reductions, and `ui_spec.yaml`). It is the case and holds nobody's data, so it needs no session. The bundle itself is not signed; its hash is, as `bundel_hash` in the source data below, and the [service profile](/components/regelhet-fsc4b-dienstprofiel) says how it is computed. Everything under the directories the manifest names is public this way, so a manifest should name no directory with data about people. The kern loads it as a source with `bundel::uit_bron`, the way the server loads its own case.

`POST /fsc4b/burger/v1/gegevens` gives the source data. The case file says which answers go out: it names them under `uitgeven`, each an answer the executor's cell gives (`answers` in its `cel.yaml`). Without that list nothing goes out, and the route answers with no answers at all. The list is the case's and not the cell's, because one cell's catalogue serves every case that runs it, and what goes out to a citizen is a choice of the procedure. Each answer named must take the login field as its only required parameter. The server fills that field with the BSN of the session, never with a value from the app, and computes the answer on today's date. A name that is no answer of the cell, or one that needs more, stops the server with a message that names it. The number of business permits a company holds needs a KvK number, and the GGD's advice needs the date of an application, so neither can be listed. For the parking card the case lists two answers: whether the municipality granted the citizen a card before, and whether a request to the GGD for an examination is open. That a listed answer is about the citizen alone rests on its reduction reading only the records of the person its parameter names, which is why nothing goes out unless the case names it.

```yaml
uitgeven:
  - aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt
  - verzoek_om_geneeskundig_onderzoek_staat_open
```

The answer holds the answers with what they were asked with and where each came from, the records of the citizen's dossier up to today, the regelwerk versions they were computed on, and the facts that only come after an application (`bij_aanvraag` of the case). Where an answer came from names the cell, the reduction and its version, but not how far the cell read its chronicle: that chronicle holds everyone's records. The app replays the records, computes the answers itself, and knows that the GGD's advice is still to come rather than missing. The whole is a JWS under the organization's key from the Stelseldocument, with the BSN in `sub`, next to the payload in plain form. Signing it lets the citizen show later where a value came from. It has no `exp` or `aud`, since it records what the organization handed out on that day, whoever the citizen later shows it to. The kern function that computes the answers is `brongegevens` in `packages/poc-chronolexografie/src/brongegevens.rs`, so an app running the kern as WebAssembly can compute them too and compare.

## Configuration

| Variable | Meaning |
|---|---|
| `REGELHET_FSC4B_OIN` | The OIN of this organization |
| `REGELHET_FSC4B_SCOPE` | The scope a credential must carry |
| `REGELHET_FSC4B_KEY_FILE` or `REGELHET_FSC4B_KEY` | The organization's private ES256 key, PKCS#8 PEM, as a file path or as the value itself. It stays out of the repository. |
| `REGELHET_FSC4B_STELSELDOCUMENT_FILE` | The Stelseldocument, a JWS |
| `REGELHET_FSC4B_STELSELBEHEERDER_KEY` | The Stelselbeheerder's public key, a JWK |
| `REGELHET_FSC4B_PUBLIC_URL` | Where apps reach the server; `BASE_URL` when not set |
| `REGELHET_FSC4B_DEMO` | `true` for the demo Stelsel below, in test mode only |
| `REGELHET_FSC4B_ORIGINS` | The origins of browser apps that call the channel from another address, comma-separated, each `https://host` or `http://host:port` |

The OIN, the scope, the key (as file or value), the Stelseldocument and the Stelselbeheerder's key come together or not at all; one of them without the others stops the server. The public URL is optional and falls back to `BASE_URL`; in stable mode it must start with `https://`. At startup the server checks the Stelseldocument's signature, takes its `app_managers` as the trusted Activatiediensten, and refuses to start when the document does not list this organization's OIN with the public key of its private key. The channel requires `REGELHET_SUBJECT_CLAIM` to be `bsn`, and it belongs on the instance for citizens: on `REGELHET_SURFACE=backoffice` its variables, the demo's included, stop the server ([surfaces](/components/regelhet#surfaces)).

`REGELHET_FSC4B_ORIGINS` is optional and needs the channel. Every answer under `/fsc4b/` carries CORS headers for those origins, a preflight, an error and a path that is no route included, so the app can read why a request failed. Other origins and the browser routes of the server get none. An origin with a path, a trailing slash or `*` stops the server.

The standard leaves the structure of a Stelseldocument to the Stelselbeheerder (§4.1.1.1). This server reads the example structure of that section, `{ version, organizations, app_managers, document_types }`, with each `public_key` as a JWK.

## The demo Stelsel

In test mode, `REGELHET_FSC4B_DEMO=true` opens the channel on a Stelsel the server makes at startup. It generates three key pairs in memory (a Stelselbeheerder, a test Activatiedienst and the organization), signs a Stelseldocument with made-up OINs, and trusts it through the same checks as a real one. Three more routes then exist:

- `GET /fsc4b/demo/stelseldocument` and `GET /fsc4b/demo/stelselbeheerder` give the document and the key it verifies under;
- `POST /fsc4b/demo/activatiedienst/v1/credential` with `{ "certificate": <self-signed JWS of the app key>, "bsn": "999990019" }` gives a credential for a day.

The test Activatiedienst checks no identity: anyone may ask a credential for any BSN from the test range (`99999xxxx`). Stable mode refuses the demo, both in the configuration and when the server starts, and on a real Stelsel the three routes do not exist.

```bash
REGELHET_FSC4B_DEMO=true just regelhet
cargo run --manifest-path packages/Cargo.toml -p regelrecht-regelhet \
  --features test-utils --example fsc4b_client -- http://localhost:7180 999990019
```

The example asks a credential, runs `CreateSession`, fetches the source data and checks their signature, submits an application and prints the dossier.

On a cluster the Helm chart `charts/regelhet` turns the demo Stelsel on with `fsc4b.enabled: true`, and `fsc4b.origins` lists the origins of browser apps. It sets `REGELHET_FSC4B_DEMO` and `REGELHET_FSC4B_PUBLIC_URL` on the citizens' instance only, and refuses the value in stable mode or when the one instance is the back office. The keys stay in memory, so the chart mounts no Secret, and after a restart an app reads the Stelseldocument again. The Fundament plugin sets the value from `spec.fsc4b.enabled` of a RegelHet and names the discovery configuration in `status.fsc4bDiscoveryUrl`. Run the example against that host, such as `https://<name>.cluster-a.fieldlab2026.nl`.

Which requests the service takes, and what goes in and comes out, is in the [service profile](/components/regelhet-fsc4b-dienstprofiel) (Dutch).

## Where the werkversie contradicts itself

Each choice sits in one place in `src/fsc4b/`.

- **The length of the session key.** §3.2, §3.4 and step 13 of §4.3.4.2.2.6 ask AES-256 with `A256GCM`. §4.3.4.1 asks the verifier to check that `session_aes_key` is 16 bytes, and its examples hold 16. A 16-byte key cannot be used with `A256GCM`, so the server requires 32 bytes (`AES_KEY_LEN`) and refuses 16.
- **The curve "P-512".** §3.1 lists P-512, which does not exist (P-521 does). The server accepts ES256 on P-256 only: the one curve phones' secure hardware offers, as §3.1 itself notes.
- **The answer of `/v1/complete`.** No text describes it; figure 10 (steps 29 and 30) shows the session token encrypted under the session key. The server answers with a JWE (`dir`, `A256GCM`) whose plaintext is the token itself.
- **Traffic after `CreateSession`.** §4.3.4 promises key material to encrypt it, and §4.4.1 shows how: a `POST` with the token in `Authorization` and a JWE under the session key as the body. The citizen's routes follow that, also for reading, so a token without its key opens nothing.
- **The specification in the discovery configuration.** §2.2.4 asks the specification of each service in the discovery configuration, while §4.2.2 allows only versions and URLs there. The configuration follows §4.2.2; the specification is at the fixed path `/fsc4b/openapi.json`.
- **Self-signed certificates.** §4.3.4.1 accepts `certificate_type_self_signed`, which carries no identity. The citizen's service needs a BSN, so the server refuses it. The demo Activatiedienst uses it to prove possession of the app key.

## What is not here

- **The post-quantum hybrid.** §3.3 recommends ML-KEM-768 with ECIES, but the challenge of §4.3.4.2.2.4 has no field for an ML-KEM key, so an app cannot use it with `CreateSession`. The server uses `ECDH-ES` alone.
- **Revocation.** The server does not check Token Status Lists (§4.5). A revoked credential is accepted until its `exp`. The Stelselbeheerder's revocation and expiry of issuer keys (§4.5, §4.6.2) depend on a toepassingsprofiel that does not exist here, so the server does not check them either.
- **Replay within a session.** The body of a request to the citizen's service holds no time or route, so a request someone captured can be sent again while the session lasts. A submission with an `idempotency_key` is answered from its receipt the second time and recorded once; that key sits inside the encryption, so it cannot be stripped off. A submission without one, and every read, can be replayed. The werkversie names no protection against this.
- **A log of handouts.** The server does not yet record that it handed out source data, although the AVG's accountability duty and the Logius standard Logboek Dataverwerkingen point toward such a log.
- **OpenID4VP and OpenID4VCI** (§4.3.5, §4.4.2), and activation of an app with a real Activatiedienst (§4.4.1).
- **A native app with a key in secure hardware.** [The app](/components/regelhet-app) runs as a desktop window with its key in software; a phone build is not made.
