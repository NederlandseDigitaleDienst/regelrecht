---
title: "Wetsproces: the app"
description: "The citizen's screens of the wetsproces server as an app in a webview, which reaches the server through FSC voor Burgers while its key and session stay in Rust."
---

The wetsproces app puts the citizen's screens of [Wetsproces Screens](/components/wetsproces-frontend) in a webview and lets them reach the server through [FSC voor Burgers](/components/wetsproces-fsc4b) instead of a session cookie. The screens are the ones in `frontend-wetsproces/src/burger/`. Only where a call goes changes: in the app it is a command of a core in Rust, which holds everything that proves who the citizen is.

It is a trial on the demo Stelsel of the server. It activates with a made-up citizen number, has been built and run as a desktop window, and has not been built for a phone.

## Where the parts live

| Part | Location | Holds |
|---|---|---|
| Protocol | `packages/fsc4b/` | The JOSE formats, the names on the wire and the client of the protocol. The server and its tests use the same code |
| Core | `packages/wetsproces-app/` | The commands, the key store, the credential and the session |
| Shell | `frontend-wetsproces/src-tauri/` | A [Tauri 2](https://v2.tauri.app/) window that registers the core's commands, and nothing else |
| Screens | `frontend-wetsproces/burger.html` and `src/burgeromgeving/` | The citizen's entry, its transport, and in `tauri/` the activation screen and the session that turns a call into a command (`kern.ts`) |

The app key, the credential, the session token and the AES session key stay in the core. The webview calls `status`, `activeren`, `afmelden`, `casus`, `formulier`, `dossier`, `waarom` and `indienen`; the capability of its window allows those eight and nothing else. Each takes and gives plain JSON, and none of them returns key material or the credential itself. The status names the citizen and when the credential lapses. None of the app's own modules uses the webview's storage.

## One entry, two transports

The citizen's entry (`burger.html`, `src/burgeromgeving/main.ts`) is the same for the app and for a web environment of citizens only. Its shell and router know a transport only through the interface in `vervoer.ts`: how a call reaches the server, how the citizen gets in, and how they leave. The build chooses the transport by mode. The import `#vervoer` resolves in `vite.config.ts` to one file, so a bundle carries one transport:

| Build | Transport | Output |
|---|---|---|
| `npm run build:app -w frontend-wetsproces` | `vervoer/tauri.ts`: commands of the core, in by activation | `dist-app/` |
| `npm run build:burger -w frontend-wetsproces` | `vervoer/cookie.ts`: HTTP with the session cookie, in by the login | `dist/burger/` |

The instance for citizens of the server serves `dist/burger/` ([surfaces](/components/wetsproces#surfaces)). The combined web build keeps its cookie through `src/gedeeld/sessie.ts`, where each transport puts its own session in place. `src/scheiding.test.ts` follows every import from the entry with each transport. It fails when either reaches a module of the back office, and when the app's reaches the login or a module under `src/` that uses the webview's storage. It does not follow imports into packages.

## Activation and the session

In test mode the activation screen offers the made-up citizen numbers of the server's test login, 999990032 and 999990020. Picking one makes the core run these steps:

1. It makes a new P-256 key and forgets any old one.
2. It reads the Stelseldocument from the demo routes of the server and checks it under the Stelselbeheerder's key, which the demo serves too.
3. It asks the test Activatiedienst a credential for the key and checks that the credential is bound to it.
4. It runs `CreateSession`.

A server without the demo answers `404` on those routes, and the app says that this server offers no demo Activatiedienst.

A session lasts at most fifteen minutes. When the service answers `401`, the core starts a new session with the same credential and sends the request again, so a form the citizen filled in goes out as it was. A submission carries its idempotency key inside the encrypted body, so the second send is answered from the receipt and recorded once. A credential that lapses within a minute, or that the server refuses, sends the citizen back to activation. A server that cannot be reached gives a message and keeps the screen as it is.

Trusting the Stelselbeheerder's key that the demo serves amounts to trusting the server itself. A real app carries that key in its build.

## Where the key lives

`KeyStore` in `packages/wetsproces-app/src/keystore.rs` is the trait the core asks for a signer and for the credential. The core only asks it to sign, so a store whose key never leaves secure hardware fits behind it. The one store built is `SoftwareKeyStore`. In the desktop app it keeps the key as a PKCS#8 file, readable by the user alone, in the app's data directory (on Linux under `~/.local/share/nl.regelrecht.wetsproces.app/fsc4b/`). No hardware protects that file. The app says so under every screen.

A store on the Android Keystore or the iOS Secure Enclave is not written. The doc comment of `keystore.rs` describes where it goes and what it converts: both platforms sign in DER, and the trait returns `r` and `s` of 32 bytes each.

## Building and running

The shell is its own Cargo workspace, outside `packages/`, so `just check` and CI build neither Tauri nor the webview libraries it needs. The core and its tests are members of the ordinary workspace. `cargo test -p regelrecht-wetsproces-app --all-features` runs the core against a real server on the demo Stelsel, from activation to the explanation of a decision. This needs no window.

On Linux the window needs the development files of webkit2gtk 4.1 (on Debian and Ubuntu `libwebkit2gtk-4.1-dev`, with `pkg-config` and `librsvg2-dev`).

```bash
WETSPROCES_FSC4B_DEMO=true WETSPROCES_PORT=7190 just wetsproces
npm run build:app -w frontend-wetsproces   # writes frontend-wetsproces/dist-app
cargo build --manifest-path frontend-wetsproces/src-tauri/Cargo.toml
WETSPROCES_APP_SERVER=http://localhost:7190 \
  frontend-wetsproces/src-tauri/target/debug/wetsproces-app
```

| Variable | Meaning |
|---|---|
| `WETSPROCES_APP_SERVER` | The address of the server. Without it the app says it has no server and activates nothing |
| `WETSPROCES_APP_OIN` | The OIN of the organization in the Stelseldocument. The demo's when not set |

The shell reads both at startup, and otherwise takes the values they had when it was built. A phone build has no environment to start from, so the build has to set them.

## On a phone (not verified)

None of the following has been run. These are the steps Tauri 2 documents for a mobile target, with the work this app adds:

1. Install the Tauri CLI (`cargo install tauri-cli --version '^2'`) and the Android SDK and NDK, or Xcode.
2. Run `cargo tauri android init` or `cargo tauri ios init` in `frontend-wetsproces/src-tauri/`, which writes the platform project under `gen/`.
3. Write a `KeyStore` on the platform's key store, as a Tauri plugin in Kotlin or Swift, and build the core with it in place of `SoftwareKeyStore`.
4. Build with `WETSPROCES_APP_SERVER` set to an address the phone can reach over HTTPS: `cargo tauri android build` or `cargo tauri ios build`.

## What is not here

- Activation with a real Activatiedienst, and OpenID4VP.
- A check of the credential's signature by the app. The server checks it on every `CreateSession`.
- Revocation, push notifications, a store release, or any deployment or release pipeline for the app.
