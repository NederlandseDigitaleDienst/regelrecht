---
title: "RegelHet: the citizen's app in the browser"
description: "A static web app that belongs to the citizen: it talks to RegelHets over FSC voor Burgers, keeps its key in the browser, and predicts a decision on the device before the citizen applies."
---

The citizen's app in the browser is a static site. It talks to every RegelHet on its list through [FSC voor Burgers](/components/regelhet-fsc4b), and it has no backend of its own. A backend would hold the keys of every citizen, and then the app would no longer be theirs. Before the citizen applies, the app fetches what the RegelHet knows about them and computes on the device what the decision would be. What the citizen types into the form stays on the device until they press the button to apply.

It runs on the demo Stelsel of each RegelHet. Activation uses a made-up citizen number, and the list of RegelHets is a file beside the app.

## Where the parts live

| Part | Location | Holds |
|---|---|---|
| Transport | `frontend-regelhet/src/burgeromgeving/vervoer/fsc4b.ts` | The third transport of the citizen's entry, beside the [desktop app's](/components/regelhet-app) and the session cookie |
| Core | `src/burgeromgeving/fsc4b/kern.ts`, `client.ts`, `jose.ts` | Activation, `CreateSession` and the encrypted calls, per RegelHet |
| Key store | `src/burgeromgeving/fsc4b/opslag.ts` | The app key and the credential, in IndexedDB |
| Kern on the device | `src/burgeromgeving/fsc4b/rekenen.ts` | The kern of `packages/poc-chronolexografie` as WebAssembly: replay, compare, predict |
| Screens | `KeuzeView.vue`, `VoorspellingPaneel.vue` | The list of RegelHets, and the prediction on the application form |
| Image and deployment | `frontend-regelhet/burgerapp/`, `charts/burgerapp/` | The static build behind nginx, and its Deployment and route |

The citizen's screens in `src/burger/` are the same as in the other two builds. The application form has one slot more (`src/burger/voorspelling.ts`): the entry fills it when its transport has a prediction panel, and only this transport has one.

## The core in the browser

The core does in TypeScript what `packages/regelhet-app/src/core.rs` does in Rust, and the screens reach it through the same commands (`status`, `casus`, `formulier`, `dossier`, `waarom`, `tijdlijn`, `uiterlijk`, `indienen`). The screens take the accent colour and title of the chosen RegelHet from its public `GET /fsc4b/uiterlijk`, and the default before one is chosen. WebCrypto does the cryptography of the protocol: ES256, ECDH-ES and AES-256-GCM. The `jose` library puts it in the compact forms. `jose.test.ts` decrypts and verifies the vectors that `packages/fsc4b/src/jose.rs` checks the server against, so the two implementations read each other's tokens.

Activation is per RegelHet. For the RegelHet the citizen picks, the app makes a new P-256 key and checks the Stelseldocument of its demo Stelsel. It then asks the test Activatiedienst for a credential bound to that key and starts a session. A session lasts at most fifteen minutes. On a `401` the core starts a new one with the same credential and sends the request again. Each RegelHet gives the app another identity, because each runs its own demo Stelsel. That ends when a shared Stelsel lists every RegelHet with a `discovery_url`.

The private key is a `CryptoKey` made with `extractable: false` and kept in IndexedDB as it is. The browser signs with it and gives nobody its bytes, this code included. It is still software, as weak as the key file of the desktop app, and the app says so under every screen. The session lives in memory only. The choice of RegelHet is kept for the tab in `sessionStorage`.

## Predicting before applying

When the application form opens, the prediction panel does four things:

1. It calls `gegevens` on the RegelHet. The answer is signed with the organization's key, and the app reads the claims from that signature. They hold the answers of the executor's cell about the citizen's BSN that the case lists for handing out, the dossier records those answers rest on, and the facts that come only after an application (`bij_aanvraag`). The [service profile](/components/regelhet-fsc4b-dienstprofiel) describes the request. A case that lists no answers hands out none; the panel says so, and the prediction then rests on the form and on what comes after the application.
2. It fetches the source of the RegelHet from `GET /fsc4b/regelwerken`. The route carries no signature of its own. The signed data name the hash of the source (`bundel_hash`), and on a source with another hash the app computes nothing. It loads the source into the kern as the one source of an empty bundle, and chooses the case the data are about.
3. It restores the records per cell as the chronicles of the case's cells, with the kern's `herstellen`. That checks each chronicle as one read from a file, and it takes decisions over as well as facts.
4. It computes the same answers with the kern's `brongegevens` and puts each beside the signed one. An answer that differs shows as an error with both values, because a prediction on chronicles that disagree with the executor's is worth nothing. The same goes for other versions of the regelwerken. A silent cell (`null`) is not the same as `false`: the panel says the RegelHet knows nothing about it.

The dossier holds only the citizen's own records, so the restored chronicles hold no one else. That this gives the executor's answers is tested natively, with a second citizen in the executor's chronicles, in `packages/poc-chronolexografie/tests/spel.rs`.

On request the panel then submits the form's values to the kern (`indienen`) and decides on trial (`voorBeslissen`). The fields the system fills come from the form the server gave: the BSN and the name from the login, the day from the RegelHet's clock.

A decision on a parking card reads the advice of the GGD, which exists only after the application. For each fact in `bij_aanvraag` the app records, on a copy, every combination of the yes-or-no fields its cell can fill in, and decides again after each. Findings have to be tried together: a passenger's card needs both a walking limitation and dependence on help (article 2 of the Regeling gehandicaptenparkeerkaart). The panel then shows the least the GGD has to establish for a grant, one line each, such as "Als GGD Wilgendaal vaststelt: langdurige loopbeperking: Toegekend", and one line for all other cases. When every combination comes out the same, it shows one outcome.

The app tries at most four yes-or-no fields per fact, sixteen trial decisions. A fact with more of them, or with a field the app cannot guess such as free text, gives a line that says it cannot be predicted. With more than one such fact, each is tried on its own and not in combination with the others.

Nothing of the trial stays in the kern. The executor sees that the citizen asked for their data, and nothing of what they typed.

## Building and running

The kern is built from Rust into `frontend-regelhet/burgerapp/wasm/pkg/`, outside the source tree. The build of the app copies it next to the bundle and stops with a message when it is missing.

```bash
REGELHET_FSC4B_DEMO=true REGELHET_FSC4B_ORIGINS=http://localhost:7150 just regelhet
just burgerapp        # the kern as WASM, then the dev server
```

The app is then at `http://localhost:7150/burger.html`. Its list is `frontend-regelhet/burgerapp/regelhets.json`, which points at `just regelhet` on port 7180. `npm run build:burgerapp -w frontend-regelhet` writes the static site to `dist-burgerapp/`. `src/burgeromgeving/fsc4b/rekenen.wasm.test.ts` plays the parking card on the real kern when the WASM build is there, and is skipped otherwise.

## Deployment

`.forgejo/workflows/burgerapp-image.yml` builds `frontend-regelhet/burgerapp/Dockerfile` on every push to main and pushes `regelrecht/burgerapp` with the tags `main` and the commit. The chart in `charts/burgerapp/` runs it as one Deployment behind an HTTPS listener on the cluster's shared Gateway, at `burgerapp.<domain>`.

The chart takes the list under `regelhets` and serves it as `regelhets.json`. It also sets the CSP's `connect-src` to the origins of those RegelHets, so the browser can reach them and nothing else. It refuses an address without TLS. Each RegelHet on the list must name the app's origin in `REGELHET_FSC4B_ORIGINS`, or the browser blocks its answers.

## What is not here

- A shared Stelsel and a real Activatiedienst. Activation is per RegelHet, on its demo.
- A check of the credential's signature by the app. The RegelHet checks it on every `CreateSession`.
- Protection against whoever serves the app. The static site comes from a server someone else runs, and whoever runs it can replace the code that uses the key.
- A prediction in which several facts after the application are combined.
