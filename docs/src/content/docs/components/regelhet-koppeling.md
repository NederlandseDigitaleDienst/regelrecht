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
| Admin panel for the linked party | Gone; the panel names the party as linked | Used by the party's own staff, from `regelhet-beslisser` upward |
| `recorded_via` of the party's acts | `koppeling` | The employee's own login |

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
| `REGELHET_LINK_PEERS` | A JSON list of the other servers: per peer its `cel`, its `url` and, pinned, its public key as a JWK under `sleutel` |
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

`GET /koppeling/v1/partijen` gives that list, with for each party whether this server is linked with it. The route exists on every instance for employees, with or without a link, and asks no login: it holds nothing the case itself does not say. A deployment reads it to decide which parties get a server. The Fundament plugin does so for a RegelHet with `parties.ownInstances` (`packages/fundament-plugin/README.md`), which is how a case without such a party ends up with one release and the parking card with two.

## The test clock

Each server of a test setup has a clock of its own. When a message taken over is dated later than the receiver's test clock, the clock moves on to that day, so the next step there does not fall before it. A reset of one server empties its chronicles and its confirmations and leaves the other server as it was, so reset both.

## What is not here

- **FSC.** The routes are shaped after the inway and outway of FSC, the standard for traffic between government organizations: one address that takes signed messages from known organizations and one worker that sends. Contracts, a directory and mutual TLS with PKIoverheid certificates are not built. FSC could replace this transport without changing what is recorded.
- **Several executors for one party.** A cell has one peer. A GGD that serves several municipalities needs a cell per municipality in its case, or a way to tell them apart inside one cell, and the corpus has one fictitious municipality.
- **A case of the party's own.** The party's server loads the executor's case and shows the part that concerns the party. It has no application of its own and no citizens' side, so a citizen sees the GGD's part of their dossier at the municipality only.
- **Decisions and copies.** Only facts cross the link. A party that decides on a submission and sends a copy of its decision, as the court does, still goes through the admin panel.
- **Encryption of the message.** The body is signed and travels over TLS. It is not encrypted to the receiver.
- **Roles on a party's server.** Its staff work from `regelhet-beslisser` upward. A reader of the party has nothing to open and lands on the login screen without a reason given.
- **A fact of a third cell.** Only records of the server's own cell go out. A fact an admin enters for a party without a server, which a linked party should receive, does not reach it.
- **A message that can never be recorded.** One the receiver keeps refusing holds up everything after it for that cell. The log of both servers names it; nothing sets it aside.
