---
title: "RegelHet: a link between servers"
description: "How an external party of a case, such as the GGD, runs on a RegelHet server of its own and exchanges records with the executor's server as signed messages."
---

A [RegelHet](/components/regelhet) server runs every cell of its case, and an admin enters what an external party did. A link takes one party out of that: the party gets a server of its own, where its staff log in, and the two servers send each other the records the other has to know. This page describes that link. For the parking card it puts the GGD on its own server, which receives the municipality's requests for an examination and sends its advice back.

The code is in `packages/regelhet/src/link.rs`. Without configuration there is no link and none of its routes exists.

## Two servers, one case

Both servers load the same case and keep a chronicle for every cell of it. Each makes only the records of its own cell: the executor's server those of the executor, the party's server those of the party. What the other cell did arrives as a message and is recorded as that cell recorded it (`kern::overnemen`), with `koppeling` as its channel and the sending cell beside it. It keeps the day the sender recorded it on, unless the receiver's chronicles already hold a later record: a chronicle only grows forward in time, so the record then takes that later day. A server's copy of the other cell therefore holds what crossed the link and nothing else.

The steps of the process do not change. The municipality's policy still records the request and, in the same commit, the GGD's receipt in the municipality's copy of the GGD's chronicle. The link then carries the request to the GGD's server, where the same two records land. The GGD's advice travels back the same way and closes the request.

| | The executor's server | The party's server |
|---|---|---|
| Who logs in | Citizens and the executor's employees | The party's employees |
| Back office | Work queue, chronicle, admin panel for parties without a server | Only what waits for the party: open requests, its decisions, its facts |
| Admin panel for the linked party | Points to the party's own system; entering here is a [fallback](#entering-for-a-linked-party-after-all) | Used by the party's own staff, from `regelhet-beslisser` upward |
| `recorded_via` of the party's acts | `koppeling` | The employee's own login |

## Entering for a linked party after all

The executor's admin panel enters nothing for a linked party. It names the party, says that it works in a system of its own, and links to that system when the server knows where it is (`voorkant` in `GET /api/backoffice/beheer/partijen`, under `gekoppeld`).

In test mode the panel also offers "Toch hier invoeren", for when the party's system does not work during a demonstration. The admin then enters the party's acts as for a party without a server, under a warning, and can hide the input again. The server lists such a party under `partijen` with `nooduitweg: true`. In stable mode it leaves the party out and refuses the routes, as before.

What the admin enters this way is recorded as entered through the admin panel, by that account. The executor's receipt of it crosses the link like any of its records, so the party's server holds the request as answered once it is reachable, and refuses a second answer. An answer the party recorded itself before that arrives is a second answer on the executor's side, and nothing reconciles the two.

## The party's own screen

A party's staff get a screen of their own (`frontend-regelhet/src/backoffice/PartijView.vue`), not the executor's admin panel. It lists the requests that wait for an answer, each with for whom it is and who asked, and under them the requests the party answered, with the day of the answer. An answer nobody asked for stays possible, below those. The bar names the party, and the test login offers the party's own accounts. `GET /api/backoffice/beheer/partijen` gives the answered requests under `beantwoord` on the party's own server only; the executor's admin panel never gets them.

On Fundament a party's RegelHet that names no accent colour of its own wears `rood`, or `robijnrood` beside an executor that is red itself, so the two are told apart at a glance. The wizard proposes the same colour.

## What is sent, and when

The chronicle is the outbox. A record of the server's own cell is due when a linked cell records a receipt of it, which the law says (`kern::ontvangers`), and it stays due until that cell's server confirmed it. Confirmations are kept in the table `link_deliveries`. A record that itself arrived over a link is never sent, since the receipt a cell makes of a message is the other half of that same message.

A worker sends what is due in the order of the chronicle. It runs after every step on its own instance and every three seconds, and each round reads the chronicles of the case in full. When a server refuses a message or does not answer, everything for that cell waits in order and is tried again; a crash between a commit and its sending loses nothing, because the commit is all there is to lose.

## A message

A message is a compact JWS, signed with ES256.

| Claim | Holds |
|---|---|
| `iss` | The cell that sends |
| `aud` | The cell it is for |
| `jti` | The id of the record at the sender |
| `iat` | The moment of sending; a message more than five minutes off is refused |
| `feit` | The fact, `<regelwerk>#<article>#<fact>` |
| `velden` | Its fields |
| `datum` | The day it was recorded on |

| Method and path | What it does |
|---|---|
| `POST /koppeling/v1/berichten` | Takes a message, the JWS as the body. A refused sender or signature answers `401` with one fixed text; the log names the check |
| `GET /koppeling/v1/sleutel` | The cell this server works for and its public key, a JWK |
| `GET /koppeling/v1/partijen` | The parties the executor asks something of by its policy, and which of them are linked; see [below](#which-parties-get-a-server-of-their-own) |

The receiver records the message under an idempotency key made of the sending cell and the record's id, in the commit that records it. A message that arrives twice is answered as the first time and recorded once.

None of these routes has a login. The first proves its sender by the signature, the second gives out a public key, and the third repeats what the case says.

## Configuration

| Variable | Meaning |
|---|---|
| `REGELHET_LINK_PEERS` | A JSON list of the other servers: per peer its `cel`, its `url` and, pinned, its public key as a JWK under `sleutel`. With `grantHash`, `url` is this server's FSC outway ([Through OpenFSC](#through-openfsc)). `voorkant` is where a person opens the peer's own screens; without it that is `url`, and behind an outway the server knows none |
| `REGELHET_LINK_PARTY` | The external party this server works for, by its cell. Not set on the executor's server. It requires `REGELHET_SURFACE=backoffice` |
| `REGELHET_LINK_KEY_FILE` or `REGELHET_LINK_KEY` | This server's private ES256 key, PKCS#8 PEM, as a file path or as the value itself |

Stable mode requires a key of the server's own, and for every peer an `https` address and a pinned public key. Test mode fills two gaps so a setup needs no key material. Without a key the server makes one at every start. Without a pinned key it reads the peer's from `GET /koppeling/v1/sleutel`, and reads it again when a signature stops verifying, as after a restart of the peer. That trusts whoever answers on the peer's address. A key read this way is kept for at least ten seconds, so a caller without the key cannot make the server ask its peer on every request.

The chart `charts/regelhet` takes the same settings under `koppeling`. A release for a party is one instance for employees (`surfaces.split: false`, `regelhet.surface: backoffice`). Of a release with an instance per side, the one for employees holds the link: it signs, it sends, and it is the address a peer names. An application on the citizens' instance lands in the shared chronicle and goes out with the worker's next round.

```bash
REGELHET_PORT=7180 \
  REGELHET_LINK_PEERS='[{"cel":"ggd_wilgendaal","url":"http://localhost:7181"}]' \
  just regelhet
REGELHET_PORT=7181 REGELHET_SURFACE=backoffice \
  REGELHET_LINK_PARTY=ggd_wilgendaal \
  REGELHET_LINK_PEERS='[{"cel":"gemeente_wilgendaal","url":"http://localhost:7180"}]' \
  just regelhet
```

## Which parties get a server of their own

The regelwerken of a case say which external parties do work of their own in the process: the ones the executor asks something of by its own policy when an application comes in (`bij_aanvraag`). For the parking card that is the GGD, which receives the request for an examination. The court is asked nothing, so it is not among them.

`GET /koppeling/v1/partijen` gives that list, with for each party whether this server is linked with it. The route exists on every instance for employees, with or without a link, and asks no login: it holds nothing the case itself does not say. A deployment reads it to check that a server for a party serves one the case asks something of.

On Fundament the party's server is a RegelHet of its own, installed through the wizard like the executor's, with its own address, namespace, colour and title. Its `spec.partij` names its cell and the executor's RegelHet. The plugin links the two once that cell is on the executor's list, and refuses a cell that is not (`packages/fundament-plugin/README.md`).

## Through OpenFSC

A peer can be reached through [FSC](https://fsc-standaard.nl) instead of directly. FSC is the standard for traffic between government organizations: each organization is a peer in a group, with an outway for the calls it makes and an inway for the services it offers, and a contract both peers signed says which outway may call which service. On the Fieldlab clusters the plugin `rinis-openfsc` installs a peer per namespace (resource `FSCInstallation`).

With a `grantHash` on a peer, `url` is this server's own outway. Every request to that peer, a message as well as its public key, goes to the outway under the header `Fsc-Grant-Hash`. The outway fetches a token from the peer's Manager and calls the peer's inway over the mutual TLS of the group, and the inway hands the request to the peer's server on the same routes as a direct call.

```json
[{"cel": "ggd_wilgendaal", "url": "http://fsc-outway-default-open-fsc-outway", "grantHash": "$1$4$..."}]
```

| | Direct | Through OpenFSC |
|---|---|---|
| Who may call | Whoever holds the peer's key | The peer a signed contract names |
| Transport | TLS to the peer's public address | Mutual TLS between outway and inway, with certificates of the group |
| Log | The server's own log | Also OpenFSC's transaction log, at both peers |
| The message | Signed (JWS) | The same, signed |

Nothing else changes. The chronicle is still the outbox, a message is still recorded once, and the receiver still checks the signature: FSC establishes which organization calls, and the signature which cell made the record. In stable mode a peer with a grant may have an `http` address, since that address is inside the cluster and the TLS sits behind the outway; its public key stays pinned.

What each side needs before the first message, all of it outside this server:

1. An `FSCInstallation` in its own namespace, with an inway and an outway, in the same group as the other side. On cluster-a the group `fsc-fieldlab` has a Directory to join (`directory.mode: External`, with its address, its peer ID and its certificate portal).
2. Its link routes registered as a service on its inway, with the in-cluster address of the instance for employees as the endpoint.
3. A contract that publishes that service in the Directory, which the Directory signs by itself.
4. A contract in which it grants the other peer's outway a connection to the service, accepted by the other peer. The hash of that grant is the other side's `grantHash`.

Steps 2 to 4 go through the Controller's screens, or through two APIs inside the namespace that take the Controller's internal client certificate (Secret `fsc-open-fsc-controller-internal-tls`):

| Step | Call |
|---|---|
| Register the service | `POST /v1/services` on the Controller's Administration API (`fsc-open-fsc-controller`, port 9444), with `name`, `endpoint_url` and `inway_address` |
| Propose a contract | `POST /v1/contracts` on the Manager's internal API (`fsc-open-fsc-manager-internal`, port 9443), with a `contract_content` of `iv` (a UUID), `group_id`, `hash_algorithm`, `created_at`, `validity` and `grants` |
| Accept a contract | `PUT /v1/contracts/{hash}/accept` on the accepting peer's Manager |

A publication grant names the Directory's peer and the service (`type: GRANT_TYPE_SERVICE_PUBLICATION`, `directory.peer_id`, `service` with `peer_id`, `name` and `protocol`). A connection grant names the service and the calling outway by the thumbprint of its public key (`type: GRANT_TYPE_SERVICE_CONNECTION`, `service` with `type: SERVICE_TYPE_SERVICE`, and `outway` with `peer_id` and `identification`). The grant's own `hash` in the stored contract is the `grantHash`. An outway takes about half a minute to learn a contract that just became valid.

How far this was tried, on 24 November 2026 on cluster-a with OpenFSC 2.9.1: two installations of ours joined `fsc-fieldlab`, one registered the link routes of a running procedure as a service and published it, and granted the other a connection. A `GET` of the key, a `GET` of the parties and a `POST` of a message then passed from the second installation's outway to that procedure, under the grant; without the header, or before the contract was accepted, the outway refused with `UNKNOWN_GRANT_HASH_IN_HEADER`. The calls were made by hand at the outway. The server's own sending under a grant is tested against a stand-in outway that requires the header and forwards the request (`tests/link.rs`), since the image on the cluster did not hold this change yet; the two have not run together.

Two things follow from the plugin as it stands. An installation is one per namespace, so two procedures in one namespace cannot be two peers. And the test group's certificate portal hands a certificate for any peer to whoever reaches it, so the group proves no identity.

The direct routes stay open while a server is linked through FSC. Closing them, so that a message can only arrive through the inway, takes a network policy or a release without a public address for those paths; neither is here.

## The test clock

Each server of a test setup has a clock of its own. When a message taken over is dated later than the receiver's test clock, the clock moves on to that day, so the next step there does not fall before it. A reset of one server empties its chronicles and its confirmations and leaves the other server as it was, so reset both.

## What is not here

- **FSC set up for you.** The server can send through an OpenFSC outway ([above](#through-openfsc)), and the Fundament plugin passes an outway and a grant per linked cell on from `spec.fsc` of a RegelHet. The installation, the service and the contracts are made by hand; the plugin makes none of them. PKIoverheid certificates are not involved.
- **Several executors for one party.** A cell has one peer. A GGD that serves several municipalities needs a cell per municipality in its case, or a way to tell them apart inside one cell, and the corpus has one fictitious municipality.
- **A case of the party's own.** The party's server loads the executor's case and shows the part that concerns the party. It has no application of its own and no citizens' side, so a citizen sees the GGD's part of their dossier at the municipality only.
- **Decisions and copies.** Only facts cross the link. A party that decides on a submission and sends a copy of its decision, as the court does, still goes through the admin panel.
- **Encryption of the message.** The body is signed and travels over TLS. It is not encrypted to the receiver.
- **Roles on a party's server.** Its staff work from `regelhet-beslisser` upward. A reader of the party has nothing to open and lands on the login screen without a reason given.
- **A fact of a third cell.** Only records of the server's own cell go out. A fact an admin enters for a party without a server, which a linked party should receive, does not reach it.
- **A message that can never be recorded.** One the receiver keeps refusing holds up everything after it for that cell. The log of both servers names it; nothing sets it aside.
