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

Chronolexografie is a static PoC in one directory, `packages/poc-chronolexografie/`: the Rust crate with its frontend in `frontend/`. A PoC whose core is a Rust crate built to WASM may use that layout; the others keep `frontend-poc-<slug>/`. It is named after the method it demonstrates, [chronolexography](/concepts/chronolexography), since it plays three cases: vakantieverlof, the gehandicaptenparkeerkaart and the bedrijfsparkeervergunning. The visitor first picks one, and the app then plays its administrative process on the cell from `packages/cel`. Vakantieverlof is an application for leave under article 13a of the Leerplichtwet 1969 at a fictional school, OBS De Klimop. The gehandicaptenparkeerkaart is an application for a disabled parking card under article 49 of the BABW at a fictional municipality, Wilgendaal. The bedrijfsparkeervergunning is an application by a company for a business parking permit at the same municipality, under a fictitious parking ordinance modeled on that of Delft (article 2 of `fictieve_parkeerverordening_gemeente_wilgendaal`). The address names the case and its scenario, so a link opens that scenario directly: `/chronolexografie/vakantieverlof/aanvraag-wordt-verleend`. The case is the name of its file, and the scenario is its name in lowercase, without accents or punctuation, with hyphens between the words.

Each case is one file in `packages/poc-chronolexografie/casussen/`. It names the cell (a directory under `corpus/cells/`), the article that establishes the application, and one output the law does not mark, described below. The app derives the rest from the law when it loads (`packages/poc-chronolexografie/src/casus.rs`):

- the regelwerken the cell runs: what the article's law reads (`source.regulation`, `source.article`, `source.decisions`), the regelwerken that implement one of its open terms, and the ones whose hooks or procedure apply to what it produces, which brings in the Awb. It follows the same references from every regelwerk it adds, and from the reductions the catalogues of the cells name (`cel.yaml`, RFC-048), which brings in the policy a reduction calls for a norm. A reduction that passes an article to the reduction it calls, such as the `basis` it gives the reduction of the Awb, reads the decisions under that article, so that article's law joins as well. The municipality's catalogue serves the parking card and the business permit, so each of the two cases also holds the other's law and the regelwerken that act on it. An override from another law adds nothing, since it only applies inside an execution that law starts (RFC-007).
- the article that establishes the objection: the one whose `produces` lists a `BEZWAAR`, Awb 7:11 for every case.
- for a case that names a court, the article that establishes the appeal (the one with a `BEROEP`, Awb 8:70), and the fact in which the cell records the receipt of a judgment. That is the one fact a regelwerk of the case requires whose parameters are all outputs of the appeal article: `afschrift_van_de_uitspraak` of Awb 8:79.
- who decides: the competent authority of the article or its law, and otherwise the law's own words in `decided_per_case_by`. That gives "het hoofd" and "het college van burgemeester en wethouders".

One thing the law does not say, so the case file says it: which output of the decision grants what was asked. `decision_type: TOEKENNING` names the kind of decision, not the output, and a decision can have more than one boolean output, as those on leave and on the parking card do. The rest of the file is for the visitor: a description, example values for the forms, and the scenarios.

A case file can also say what the cell knows before the first scenario (`begin`). No step of the process records it, yet the law asks for it. For vakantieverlof that is one executogram dated 1 August 2026 (RFC-051), which places the school in Utrecht, so region midden. The app records it through `Cell::record_executogram` whenever the cell starts, so the cell checks it as any step, and it shows in the chronicle and the log. The school's own holidays need no such step, since they come from its policy. The parking card and the business permit have nothing to record first in the municipality's own cell. A permit granted earlier cannot come from the case file either. The municipality's cell starts before the other cells, so such a decision would ask the Kamer van Koophandel (Chamber of Commerce) before the Chamber's cell holds any registration. The scenarios grant it in play instead.

A case file can list the other cells the case takes along (`andere_cellen`, RFC-048), each with its own `begin`. The parking card takes two: the GGD Wilgendaal and the court (see below). The GGD has examined the example applicant, and its advice finds a long-term walking limitation. It has also examined a second applicant, whose advice meets no criterion. Every step of the app passes these cells along with the case's own cell, so the engine can ask the one the law makes competent for a question. The tab Andere cellen lets such a cell record an instance of a fact, such as the GGD recording an examination or an advice, and the chronicle tab shows every cell's chronicle. A switch left off records `false`, so an advice with yes-or-no fields can be recorded at all.

The business permit takes two other cells along: the court, and the Chamber. The Chamber has registered three fictitious companies, each with the postcode of its establishment and the number of people who work there, as executograms of the fact `vestiging_ingeschreven`. A fictitious policy of the Chamber requires that fact (`fictief_beleid_handelsregister_kamer_van_koophandel`). The [open questions](/concepts/chronolexography/open-questions) explain why the Handelsregisterwet 2007 itself does not. The Chamber's reduction answers with the registration that holds on the day of the decision. The fictitious implementing decision of Wilgendaal binds its open terms to those answers. It binds the count of valid permits to the municipality's own reduction, which reads the decisions that hold from the reduction of the Awb.

The tabs are role-neutral. Aanvrager files the application, the objection and the appeal; Bestuursorgaan decides, under the name the law gives the role. The other tabs show the chronicle, the cell's reductions and a log. The tab Reductie shows each reduction with its text, as written, and its path under `corpus/cells/`. It also shows the answers the cell gives from it and, per answer, the policy article that binds it to an open term (RFC-048). For each reduction it names the policy or reduction that reduction calls for a norm. The log lists every step the cell recorded, newest first, together with the facts the other cells of the case recorded, such as a registration by the Chamber or an examination by the GGD. The decisions of another cell stay out of it, because their reasons belong to that cell. For a decision it shows why, condensed from the engine's trace: which articles the decision called, and who filled each open term, down to the answer the cell gave from its own chronicle. Where a regulation filled a term and the cell answered a term inside that regulation, as with the municipality's policy on a second card, the log follows the regulation down to that answer. The log keeps nothing of its own. It reads the chronicle, and the cell takes each decision again to show why. A step the cell refused recorded nothing, so it is not in the log.

In the chronicle and the log each row opens in place (an `nldd-list` of type `tree`). In the chronicle it shows what a chronolexogram refers to, the findings of a decision, and its fields and outputs. In the log it shows the reason for a step, what it led to, and for a decision the chain of the engine's trace as branches below it. The chronolexogram as JSON and the full trace open in a sheet. Only the page scrolls; the guide beside it is a pane of its own. When the portal serves the app, it puts the strip with the status and the voorbehoud above the app in the body. `base.css` makes the body a column in which the app takes the height that is left, so the document does not scroll next to the page.

The tab Scenario's sums up each scenario of the three cases in one sentence. "Begin dit scenario" resets the chronicle and opens a guide beside the page. The title, the tabs and the play date sit in bars across the full width, as in the editor; under them a split view holds the page and, as its inspector, the guide. On a narrow screen the split view turns the guide into a sheet, and the button Uitleg brings it back after Verbergen. The guide lists the steps with the tab and play date of each, and shows what the visitor now sees and what to do next, with a button to the right tab and one that moves the play date forward. Once every step is done, the guide shows that scenario's `eind`.

A scenario sits whole in its case file. Each step names what the visitor does (`stap`), its play date, the values its form takes (`invoer`), what it comes to (`uitkomst`) and the text of the guide. `packages/poc-chronolexografie/frontend/src/scenarioGuides.js` turns that into the guide. It adds the tab of each kind of step, a title and a text where the step gives none, and when a step counts as done. The guide names the values of a step, and the form of that step holds the same values: the case's example values with those of the step over them, under a hint that they come from the scenario (`scenarioInvoer.js`). The visitor can change them. In free play every form starts empty. `packages/poc-chronolexografie/tests/scenarios.rs` plays every scenario with exactly those values and checks the outcome of each step.

For a demo the choice screen starts with a deck and three scenarios as cards. The button Presentatie starten covers the page with an introductory deck in Rijkshuisstijl blue, and `?presentatie` in the address opens it too (`?presentatie=5` at slide 5). The deck explains the terms of chronolexography first. It then follows the parking card from article 49 of the BABW through the municipality's policy to its reduction and that of the Awb, and ends with how the decision on the card takes the answers of the GGD and the municipality together. The closing slide offers three scenarios of that case: a card granted on the advice of the GGD, a second card next to a valid one, and a card granted on objection that counts at the next application. Choosing one closes the deck and begins that scenario with its guide. The slides are data in `packages/poc-chronolexografie/frontend/src/presentation/slides.js`. Its test checks that every name a slide sets in code exists in the corpus. A YAML fragment names the file it comes from, and the test finds its lines there in order.

Next to it, the button Hoe het in elkaar zit replaces the choice screen with an explanation page, also at `?uitleg`, and a button on it leads back. It follows one application for a business parking permit, written for a reader new to the law format: from article 2 of the ordinance through the implementing decision of Wilgendaal, the cell's catalogue and its reduction to the reduction of the Awb, with a side step to the Chamber of Commerce. A button at the bottom opens that case. Each step is a section with a short heading, two or three sentences and one or more panels of YAML. A panel shows a whole file of the corpus as a tree that folds, from its root, the way the demo shows a law (`YamlNode` in `packages/frontend-shared`). The parts the step is about start open, and the reader can open the rest. Where a step rests on an article, it first quotes the text of that article. The page is written by hand in `packages/poc-chronolexografie/frontend/src/presentation/uitleg.js`, which names per panel the file and the paths that start open. `tests/uitleg.test.js` checks that each of those paths exists in its file and that every quote stands word for word in the `text` of its article, so a change in the corpus that the page no longer matches fails the test.

The guide reads the visitor's progress from the chronicles, so free play between steps still counts. A refused step leaves nothing in a chronicle. The guide therefore remembers the cell's refusals until the visitor chooses a case, a scenario begins or a reset shortens the chronicle. The scenario with a second objection against a replaced decision ends on such a refusal. A step that departs from the scenario, such as a decision that grants where the scenario expects a rejection, gives a warning and does not block. The NAPP case has no guide and shows the steps from its case file instead.

Every case file of this crate lists a court among its other cells, `rechtbank_wilgendaal`, with a chronicle of its own (RFC-053). The case file does not say that this cell is the court. The app finds it as the one other cell whose `cel.yaml` names who decides on the appeal article: "de bestuursrechter", from `decided_per_case_by` on Awb 8:70. For vakantieverlof it is the only other cell. Under Andere cellen the court gets its own section instead of a form for facts. There it records an appeal with the copy of the decision on objection that the case's cell made. It gives judgment with its findings and sends a copy back, and the case's cell records the receipt.

The findings come from the law: the open terms the court judges on Awb 8:70, and on 8:72, which 8:70 reads only for a well-founded appeal. When the court decides the case itself (8:72 lid 3 onder b), it takes the decision on objection anew, so it also gives the findings of that decision and of the case's decision. The app leaves out a finding the judgment does not read, and names it. After an order to decide again, Bestuursorgaan decides on the same objection once more, under Na een uitspraak. The court's judgment and that new decision on objection get the same preview as a decision, described below: `voorvertoningUitspraak` takes the judgment on a copy of the court's cell, and `voorvertoningNaUitspraak` the decision after a receipt. The chronicle tab shows the court's chronicle with the others. Vakantieverlof plays a well-founded appeal, the parking card an unfounded one.

Each decision shows why it came out as it did, as the cell explains it (`Cell::explain`): who filled each open term, and the engine's trace under the law in force. The deciding authority is never asked what the cell answers from its chronicle through a reduction, such as whether the pupil already had leave this school year, whether the leave falls outside the school holidays, or whether the applicant already holds a valid card, as long as the cell knows the answer for that case. In the trace that answer is a single step under the binding, which names the answer, the reduction with its article and version, and the last chronolexogram the cell read. The log names the binding first ("beleid ... art. 2, antwoord ... van OBS De Klimop (reductie ...)"). Asked how it got there, the cell runs the reduction again and shows its own trace (`Cell::explain_answer`), with the steps of the policy it calls. For earlier leave that trace shows the call to the reduction of the Awb. That reduction reads the decisions under 13a from the school's chronicle and gives those that hold after objection and appeal; the trace also shows how many of them the filter skipped. For the holidays the trace lists what the reduction read from the Regeling vaststelling schoolvakanties 2025–2030, such as the region and the Christmas holiday, and the place of establishment the cell answered inside that reduction, which can be explained in turn. The tab Reductie lists what the reduction of the Awb reads, under the reduction that calls it.

Beside the findings it gives, or below them on a narrow screen, the deciding authority sees a preview headed "Voorlopige uitkomst · nog niet vastgelegd". It shows the outcome the decision would have, with its trace in the condensed form the log uses. The app asks again 300 ms after the last change to a finding, the play date or the chronicle. It drops any answer that arrives after a newer change. The app takes the decision on a copy of the cell with its chronicle copied into memory (`Cell::sandbox`). The outcome and the trace are those of the real decision, and nothing is recorded. `voorvertoningBesluit` does this for an application. `voorvertoningOpBezwaar` does it for an objection once its admissibility is tested. That preview also shows whether the objection is upheld and which new decision would follow. Both take the arguments of the step they preview. When the cell refuses, for instance because a finding is still missing, the answer is `{ "geweigerd": ... }` and not an error. The panel then gives the reason in Dutch through the translations the banner uses (`melding.js`), or the cell's own message where there is none. Any other error fails as in any step, and the panel says the preview did not work, with the message.

The preview also tells the form which terms to ask. A reduction can be silent for a case and answer null. The school's holiday policy (`fictief_beleid_vakantierooster_obs_de_klimop`) does that for leave in school year 2027–2028, which it does not cover yet. The head's policy then says that the head judges the case. It does so with an open term of its own (`verlof_valt_buiten_schoolvakanties_volgens_hoofd`, RFC-048 §2). The parking card's policy does the same for the four medical criteria when the GGD gave no advice. The form first asks only the findings of the deciding article itself. Whether a case needs one of the policy's own open terms, the app can only see by running the decision. The app first takes the decision on the copy without a finding on any of them. A term the engine then reports as missing is one the policy leaves to the authority for this case. The engine names one term per run, so the app runs again with the finding on it to find the next, and stops at the first one not judged yet. Until then the answer is `{ "geweigerd": ... }` with that missing judgment. The answer lists those terms in `stil`, each marked `beleid`, and the form shows each as a finding whose note says the policy leaves it to the authority for this application. A term the cell does answer stays off the form and appears in the trace as filled in by the policy with the cell's answer. The objection form works the same way.

The app asks the engine which open terms a decision may need and who answers each one (`LawExecutionService::decision_terms`). The cell adds the parameters, which it takes from the application, and the date of the decision (`decision::derive`). The app itself decides when to ask for them, in the form and in the preview (`kern.rs`).

A third case, a fictional subsidy at the Nederlandse autoriteit politieke partijen, does not live in this repository. Its case file, its regulation and its cell are on a branch of the private NAPP corpus, and the app loads them through the portal when the page opens. The next section describes how.


#### In the browser

Everything runs in the visitor's browser. The cell, the engine and the app's core (`kern.rs`, `casus.rs`) compile to WebAssembly in one crate, `packages/poc-chronolexografie` with the feature `wasm`. No server holds state. Every browser tab holds its own game, so two visitors play their own case at the same time and a reset in one tab leaves the other alone.

`packages/poc-chronolexografie/src/spel.rs` holds one game: the cases to choose from and the chosen one with its cells. `src/wasm.rs` next to it puts it behind wasm-bindgen as the class `Poc`, with one method per step of the game, from choosing a case to a reset. Each method answers in JSON text, and a refusal of the cell is a thrown `Error` with the cell's message. The components call only `packages/poc-chronolexografie/frontend/src/api.js`, which forwards each call to `Poc`. A panic in the cell aborts the WebAssembly instance; the app then says the cell stopped and that a reload goes on from the last step that worked.

The cell reads its directory through the trait `Files` in `packages/cel/src/files.rs`. `Disk` reads the file system, for the tests and the tools; `Bundle` holds the files in memory. The browser gets the bundle as `bundel.json`: every file the cases need, by its path in the repository, without the scenarios. Which directories that takes is listed under `corpus` for `chronolexografie` in `pocs/registry.yaml`, and `packages/poc-chronolexografie/frontend/scripts/copy-assets.js` builds the bundle from that list. Each case file lists its own corpus under `corpus`: the regelwerken that count in that case and no others, each a directory under `corpus/`. The reductions of the case's cells come with those cells. A regelwerk outside that list neither runs nor acts on the case, even with a hook on every decision, so a new regelwerk joins a case only when its file names it. `packages/poc-chronolexografie/src/bundel.rs` collects the lists of all cases with their cells. Its tests check that the register lists exactly that, that the bundle plays every case as the whole corpus does, and that `script/deploy-filters.mjs` watches each directory.

The chronicle of every cell lives in memory. After each step `packages/poc-chronolexografie/frontend/src/state.js` saves the chosen case, the chronicles, the play date, the scenario the visitor began and the refusals since then in `sessionStorage`. A reload of the tab then goes on where it was, as long as the address names the saved case. The start address shows the choice of case, even when the tab saved a game; otherwise a demo scenario begun from the deck's closing slide would skip the start screen on every later visit. `Cell::restore` checks the saved chronicles as `Cell::open` checks a chronicle file. That storage belongs to the tab, so two tabs never overwrite each other's game. Without storage, in a private window or with a full quota, the game plays on and a reload starts at the choice of case. A saved game the cell refuses, for instance after the law changed, is dropped with a message.

#### Sources

A source is a corpus that a static PoC loads while it runs rather than at build time. The register lists it under `sources` for the PoC, with a GitHub repository, a branch, a folder on that branch and the name of the environment variable that holds the token. For chronolexografie that is `napp`: the manifest `werkbank/chronolex/napp/bron.yaml` of the branch integratie/analistenportaal in the private NAPP corpus, read with `NAPP_CORPUS_TOKEN`. A push to that branch reaches the PoC within a minute and needs no build of regelrecht.

The portal serves each source at `/<slug>/bron/<name>`, behind the PoC's own password (`packages/poc-portal/src/bron.rs`). It fetches the branch as a tarball from the GitHub API and keeps the YAML files under the folder. The answer is one JSON object with the files by their path in the source, plus the short sha of the commit. The token stays in the portal and never reaches the browser. A fetch is kept for 60 seconds. When GitHub fails after that, the portal serves the last good copy with a message saying so. Without a copy it answers `502`, and without a token `503`, each with the reason as text.

Instead of a folder, `path` may name a manifest, a `.yaml` file on the branch. A manifest makes the source from directories anywhere on the branch, so a repository laid out for other work serves as a source without moving anything. Under `mappen` it gives, per directory of the source, the directory of the repository it comes from, counted from `wortel`. `wortel` is the repository root as seen from the manifest's own directory, so the manifest reads the same on the branch and in a local checkout. A file belongs to the longest directory that holds it: a cell's `reducties` can land under `reductions` while the rest of the cell lands beside it. Only what the manifest names reaches the browser. A directory without a YAML file is an error that names it.

The app reads the list of sources from `bronnen.json`, which `copy-assets.js` writes from the register, and fetches each one next to `bundel.json` (`frontend/src/bronnen.js`). A source holds the same three directories as the corpus: `casussen/`, `regulation/nl/` and `cells/`. `cells/` also holds the reductions, those of each cell and those of none. The existing cases never see a source, which keeps a hook in a NAPP regulation out of vakantieverlof (`met_bronnen` in `src/bundel.rs`).

Loading refuses a `REDUCTIE` under `regulation/nl/`, anything else in a directory of reductions, a reduction whose directory is not named by its `$id`, and a reduction that reads one of another cell. Two directories in one source with the same `$id` are an error. A case from a source runs on that source alone, one corpus at a time: it brings its own Awb and every other regelwerk it needs, and nothing of the regelrecht corpus takes their place. A case id that is already taken is set aside, and the app says so.

A source that is not available never stops the app. That covers a missing token as well as a failed fetch. The choice screen shows a banner, "Bron napp niet beschikbaar", with the reason, and the other cases play as before. A file in a source that does not parse drops the whole source. The banner then turns red, reads "Bron napp laadt niet" and names the file. A single case that does not load only drops that case. These errors appear when the page loads, not in the CI of regelrecht, which never sees the source.

When this page was written (October 2026), the WASM file was about 3.1 MB (0.9 MB gzipped) and the bundle about 380 kB. The portal serves both uncompressed, as it does the engine of the other static PoCs.

### The gate

One shared password per PoC, read from `POC_PW_<SLUG>` (the slug in capitals, hyphens as underscores). The portal will not start when a PoC in the register has no password, since that PoC would otherwise be open to anyone.

The passwords are user env vars on the ZAD component `poc`, set in the ZAD Operations Manager; zad-cli has no command for them. A preview copies the env vars from production when it is created, so a preview made before a password existed fails to start with `NoPassword`.

After a correct password the portal sets an HMAC-signed cookie that names the slug and an expiry. The signing key is `POC_COOKIE_SECRET`, separate from the passwords so that rotating one password leaves the other PoCs' cookies valid. The cookie only says "this browser knew the password"; a PoC's own login, such as napp's, sits behind it unchanged. The redirect after signing in stays under the PoC's own prefix, and the content security policy is `POC_CSP` from `packages/auth/src/security_headers.rs`. The details are in `gate.rs`.

### The policy assistant

A static PoC can have `assistent: true`. The image then also carries `packages/poc-assistent/`, a Node server that runs the Claude Code CLI headless with MCP tools specific to one case, and `start.sh` starts one process per case on localhost, from port 3600 upward, and exports `POC_ASSISTENT_<SLUG>` so the portal knows where to forward `/<slug>/api/...`. Without a `CLAUDE_CODE_OAUTH_TOKEN` or `ANTHROPIC_API_KEY` no assistant starts, `/api` answers `503`, and the app hides its assistant panel.

The list of cases comes from `POC_ASSISTENT_CASUSSEN` in `start.sh`, not from the register; its default is `terugbetaalregimes nieuwkomersbekostiging`. A new PoC with `assistent: true` therefore also needs that default or the variable changed.

### Image and deployment

`packages/poc-portal/Dockerfile` builds the engine and chronolexografie to WASM, the portal's design-system bundle from `frontend-poc-portal/`, each static PoC with its own base path, and the Rust binary. The portal and napp deploy as two ZAD components, `poc` and `napp`; see [Deployment](/operations/deployment). Only `poc` is published on the web. The image builds WASM from two crates the portal does not depend on: the engine and `regelrecht-poc-chronolexografie`. `script/deploy-filters.mjs` therefore ties the `poc` component to those two and to the portal binary, so a change to the engine or the cell rebuilds the image. The directories the chronolexografie bundle takes from `corpus/` are listed there too.

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
