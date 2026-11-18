---
title: "Shared Frontend Package"
description: "The npm workspace package with the code the Vue frontends share: API calls, sign-in state, the color scheme, and the Gherkin runner the editor, the demo and the PoCs run scenarios with."
---

`@regelrecht/frontend-shared` holds the frontend code that more than one Vue app needs. It is not built or published on its own: the apps import its source files through the npm workspace, and Vite bundles whatever they import.

## Overview

- **Language**: TypeScript and Vue 3 single-file components with `<script setup lang="ts">`; only the generated Gherkin grammar is JavaScript
- **Location**: `packages/frontend-shared/`
- **Type**: npm workspace package (`private`, not published)
- **Tests**: Vitest with happy-dom, `npm test -w packages/frontend-shared`
- **Type check**: `vue-tsc`, `npm run typecheck -w packages/frontend-shared`

## What it does

The editor is the reference for what lives here, and the other apps conform to it. Which app uses what:

| App | Imports |
|-----|---------|
| Editor (`frontend/`) | API calls, sign-in state, the GitHub link status, the color scheme, value helpers, the Gherkin runner |
| Demo (`frontend-demo/`) | The color scheme, value helpers, the Gherkin runner |
| The two static PoCs (`frontend-poc-*`) | The Gherkin runner, saved state, browser variants, the line diff, the reload-on-stale-bundle handler, the tool-call formatter, the shared components |
| Lawmaking (`frontend-lawmaking/`) | The color scheme |

## Architecture

The package root (`src/index.ts`) exports only the small primitives. Everything else has its own subpath in the `exports` map of `package.json`, so an app pulls in only what it imports: `./gherkin`, `./lib/*`, `./components/*` and the individual modules as `./*`, without an extension. The one JavaScript file, `./gherkin/grammar.generated.js`, keeps its extension.

### Exported from the root

| Module | Purpose |
|--------|---------|
| `apiFetch.ts` | A thin wrapper around `fetch`. A non-ok response throws an `ApiError` carrying the status, the body text and the content type. It deliberately has no retries, timeouts or auth handling; `apiFetch` returns the raw `Response` so callers keep the headers the ETag/If-Match handling relies on |
| `useAuth.ts` | Sign-in state from `/auth/status`, the realm roles of the current user, and `hasRole` / `hasAnyRole` for showing role-specific UI. The backend still enforces every route |
| `useGithubAuth.ts` | Whether the user has linked a GitHub account, from the editor API's `/auth/github/status`. When the endpoint is missing or fails, the controls are hidden |
| `useColorScheme.ts` | The light/dark/auto picker. It sets `data-scheme` on `<html>`, which the design system keys its dark tokens on, and leaves it off for `auto`. Persistence is injected: the editor stores the choice server-side, other apps use `localStorage` |
| `values.ts` | `isUnknown` and `missingFacts`: one definition of how the engine's Unknown value ([RFC-036](/rfcs/rfc-036)) arrives in JavaScript |

### TypeScript

The package is written in TypeScript. As in the editor, `tsconfig.json` references two programs: `tsconfig.app.json` for the package code, and `tsconfig.node.json` for the tests, their support code and the scripts, which run under Node and get its types. The app config extends `@vue/tsconfig/tsconfig.dom.json` (strict mode) and sets `allowJs`, which lets the TypeScript modules import `grammar.generated.js`; that file stays JavaScript because `bdd/codegen/gen-js.mjs` writes it and `docs/scripts/check-gherkin-steps.mjs` loads it with plain Node. `steps.ts` checks its entries against the `GrammarEntry` type.

- Nothing is compiled ahead of time: the apps' Vite builds and Vitest strip the types when they bundle or test the source. Imports leave out the extension, so a script run with plain `node` cannot load these modules. The other conventions all frontends share are in [Frontend Code](/guide/frontend-code).
- The Gherkin runner names the engine methods it calls in a small `GherkinEngine` interface. The engine's own generated `.d.ts` is part of each app's WASM build output, which this package cannot reach.

### The Gherkin runner (`./gherkin`)

A JavaScript runner for the canonical scenario language in `bdd/grammar.yaml`, run against the WASM build of the engine. The editor's scenario builder and the demo's scenario page both run on it.

| Module | Purpose |
|--------|---------|
| `parser.ts` | Wraps `@cucumber/gherkin` into a small AST of scenarios, steps and data tables |
| `grammar.generated.js` | The step patterns, generated from `bdd/grammar.yaml` by `just bdd-codegen`. Never edit it by hand; CI regenerates it and fails when the committed file differs |
| `steps.ts` | Turns the generated grammar into step definitions and matches step text against them |
| `actions.ts` | The one dispatch that gives each grammar action its behavior, the JavaScript counterpart of the Rust BDD dispatch |
| `context.ts` | `ExecutionContext`, the state of one scenario run |

The runner supports only the `core` tier. A step from another tier throws instead of doing nothing, so a scenario that needs more than the WASM engine offers fails visibly.

### Modules for the PoCs

These came in with the proof-of-concepts behind the [PoC Portal](./poc-portal). Their code is in English; the text they show to users is Dutch.

| Module | Purpose |
|--------|---------|
| `usePersistedState.ts` | Keeps choices and settings across a page reload in `localStorage`, under a prefix per case. Two PoCs share one origin behind the portal, and without the prefix they would read each other's state. An edited rulework is deliberately not stored here |
| `browserVariants.ts` | Variants a user saves in their own browser, in the same shape as the variants checked into the case. Each file keeps a fingerprint of the law text it started from, so the app can say when the corpus has changed underneath it. Variants saved before the code was translated carry Dutch field names; they are read under both names and written back in the new format |
| `reloadOnStaleBundle.ts` | Reloads the page when a lazily loaded view no longer exists after a deploy, at most once per ten seconds |
| `formatToolCall.ts` | Renders a tool call in the policy assistant's feed as one readable line, whatever shape the model's arguments take. Used by `AssistentPanel.vue` in both static PoCs |
| `lib/diff.ts` | A line diff (longest common subsequence) for showing a change to a law's YAML, with no dependency |
| `components/` | `Panel.vue` (a collapsible section), `ColumnMenu.vue` (a menu to pick variants as columns), `AssistantNotices.vue` (notices from the policy assistant) and `OptimizationPathChart.vue` (the path the assistant takes toward a target, which needs the optional `echarts` peer dependency) |

### RegelRijm (`./regelrijm/index`)

RegelRijm reads an action tree as Dutch: `[hoogte zorgtoeslag] wordt:
afgerond op hele centen ( het deel van … boven … )`. It is generated from the
YAML each time a rule is shown, never written by hand, and the same tree always
gives the same text. The grammar, the choices that keep two trees from reading
alike, and the editor pane are in [RFC-054](/rfcs/rfc-054).

| Export | Purpose |
|--------|---------|
| `renderAction(action, { article, glossary })` | The full reading of one action, as text and as a token stream for HTML |
| `renderFocus(action, path, { article, depth, open })` | The reading of one operation, with operations deeper than `depth` folded into their signature, `⟨bedrag uit [a] en [b]⟩`: what the part gives and which inputs it reads. `open` holds path keys (`pathKey`) of folded parts to show one level deeper |
| `articleContext(article, glossary)` | The declared units, the vocabulary and the article's constants, built once per article |
| `untranslated(segments)`, `references(segments)` | The names a reading shows without a term, and every name it refers to |

A name without a term reads as `[humanised name]`; a glossary (`name: term`)
is optional and can be passed in by the caller, so a corpus kept elsewhere keeps
its words out of this package. The tests next to the module read every action in
`corpus/regulation` back to its tree with a parser that exists
only in `test-support/`. `npm run regelrijm-report -w packages/frontend-shared -- corpus/regulation`,
run from the repository root,
reports per corpus how many rules read on one line and which names have no
term. The script is TypeScript and runs through `tsx`, which resolves the
extensionless imports that plain Node cannot.

## Further reading

- [Editor](./frontend) - the main consumer, and the reference for this package
- [Demo](./demo) - runs scenarios in the browser with the Gherkin runner
- [PoC Portal](./poc-portal) - the PoCs that use the saved-state and variant modules
- [Scenarios](/concepts/scenarios) - the scenario language the runner executes
