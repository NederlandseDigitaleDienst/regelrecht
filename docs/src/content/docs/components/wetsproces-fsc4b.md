---
title: "Wetsproces: FSC voor Burgers"
description: "How the wetsproces server offers its citizen routes to an app that proves itself with a Verifiable Credential, after a werkversie of the FSC voor Burgers standard."
---

The [wetsproces](/components/wetsproces) server can offer its citizen routes as a service after the standard FSC voor Burgers. An app on a phone then proves who the citizen is with a Verifiable Credential and a key it holds, where a browser shows a cookie. This page describes that channel of this one component. It implements a werkversie of the standard, the one at [docs.fsc4b.nl/specs/protocol](https://docs.fsc4b.nl/specs/protocol/) as read in November 2026; section numbers below are that document's. Employees keep logging in through Keycloak, and the browser login for citizens stays as a second channel.

The code is in `packages/wetsproces/src/fsc4b/`. Without configuration the channel is off and none of its routes exists.

## What an app does

1. It makes a P-256 key pair, on a phone in its secure hardware.
2. It gets a credential for that key from an Activatiedienst: a JWS with the app's public key in `cnf`, a `scope`, the Activatiedienst's OIN in `iss`, a validity period and the citizen's `bsn` and name (§4.3.1).
3. It reads the Stelseldocument, checks the Stelselbeheerder's signature on it, and takes this organization's public key and discovery URL from it. The discovery configuration names the two services (§4.2).
4. It runs `CreateSession` (§4.3.4). `POST /fsc4b/session/v1/challenge` gives a JWS, signed with the organization's key, with a nonce and an ephemeral P-256 key. The app signs the nonce for this organization's OIN (`aud`), puts that signature, the credential and a fresh AES key in a Verifiable Presentation, encrypts it to the ephemeral key (`ECDH-ES`, `A256GCM`), and sends it with the nonce to `POST /fsc4b/session/v1/complete`. The answer is a session token, encrypted under the app's AES key.
5. It calls the citizen's service with the token in the `Authorization` header and a body that is a JWE under the session key (`alg: dir`, `enc: A256GCM`). The answer comes back encrypted under the same key. Only a `401` (no live session under the token, or a body that does not decrypt under its key) and a `500` while reading the session are plain JSON.

The reference client in `src/fsc4b/client.rs` (feature `test-utils`) does each step as its own function. It is what the end-to-end tests in `tests/fsc4b.rs` run on.

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
| `GET /fsc4b/discovery` | The discovery configuration: `sessionApi` and `wetsprocesBurgerApi`, each in version `v1` |
| `GET /fsc4b/openapi.json` | The OpenAPI 3 specification of these routes |

The citizen's routes run the same handlers as the browser routes. A session of this channel is a citizen and nothing else: it holds the role `wetsproces-burger` and the credential's BSN under the subject claim, never an employee's role, and it reaches no route outside `/fsc4b/burger/`. The store keeps the channel beside every record, in `recorded_via` (`keycloak`, `testlogin` or `fsc4b`), and the account as `fsc4b:` followed by the RFC 7638 thumbprint of the app's key.

A refused presentation answers `401` with one fixed message, whatever check failed; the server log names the check. The log never holds a nonce, a token or a key. A session is kept under the SHA-256 of its token. Challenges and sessions live with the chronicles, in memory or in the tables `fsc4b_challenges` and `fsc4b_sessions` of the schema `wetsproces`. Taking a challenge deletes its row in the same statement, so two instances of the server never both accept one nonce. About 10,000 challenges wait at once, over all instances; above that `/v1/challenge` answers `503`. The bound is checked without a lock: challenges started at the same moment can each pass the check, so the count can go over by as many as run at once. A limit per client address is left to the ingress in front of the server.

The ephemeral key of a waiting challenge and the AES key of a session are stored as they are, since another instance must be able to use them. Whoever can read the database, or a backup of it, while they live can decrypt the traffic of that challenge or session.

## Configuration

| Variable | Meaning |
|---|---|
| `WETSPROCES_FSC4B_OIN` | The OIN of this organization |
| `WETSPROCES_FSC4B_SCOPE` | The scope a credential must carry |
| `WETSPROCES_FSC4B_KEY_FILE` or `WETSPROCES_FSC4B_KEY` | The organization's private ES256 key, PKCS#8 PEM, as a file path or as the value itself. It stays out of the repository. |
| `WETSPROCES_FSC4B_STELSELDOCUMENT_FILE` | The Stelseldocument, a JWS |
| `WETSPROCES_FSC4B_STELSELBEHEERDER_KEY` | The Stelselbeheerder's public key, a JWK |
| `WETSPROCES_FSC4B_PUBLIC_URL` | Where apps reach the server; `BASE_URL` when not set |
| `WETSPROCES_FSC4B_DEMO` | `true` for the demo Stelsel below, in test mode only |

The OIN, the scope, the key (as file or value), the Stelseldocument and the Stelselbeheerder's key come together or not at all; one of them without the others stops the server. The public URL is optional and falls back to `BASE_URL`; in stable mode it must start with `https://`. At startup the server checks the Stelseldocument's signature, takes its `app_managers` as the trusted Activatiediensten, and refuses to start when the document does not list this organization's OIN with the public key of its private key. The channel requires `WETSPROCES_SUBJECT_CLAIM` to be `bsn`.

The standard leaves the structure of a Stelseldocument to the Stelselbeheerder (§4.1.1.1). This server reads the example structure of that section, `{ version, organizations, app_managers, document_types }`, with each `public_key` as a JWK.

## The demo Stelsel

In test mode, `WETSPROCES_FSC4B_DEMO=true` opens the channel on a Stelsel the server makes at startup. It generates three key pairs in memory (a Stelselbeheerder, a test Activatiedienst and the organization), signs a Stelseldocument with made-up OINs, and trusts it through the same checks as a real one. Three more routes then exist:

- `GET /fsc4b/demo/stelseldocument` and `GET /fsc4b/demo/stelselbeheerder` give the document and the key it verifies under;
- `POST /fsc4b/demo/activatiedienst/v1/credential` with `{ "certificate": <self-signed JWS of the app key>, "bsn": "999990019" }` gives a credential for a day.

The test Activatiedienst checks no identity: anyone may ask a credential for any BSN from the test range (`99999xxxx`). Stable mode refuses the demo, both in the configuration and when the server starts, and on a real Stelsel the three routes do not exist.

```bash
WETSPROCES_FSC4B_DEMO=true just wetsproces
cargo run --manifest-path packages/Cargo.toml -p regelrecht-wetsproces \
  --features test-utils --example fsc4b_client -- http://localhost:7180 999990019
```

The example asks a credential, runs `CreateSession`, submits an application and prints the dossier.

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
- **OpenID4VP and OpenID4VCI** (§4.3.5, §4.4.2), and activation of an app with a real Activatiedienst (§4.4.1).
- **The native app**, its deployment and a frontend.
