---
title: "Frontend Code"
description: "How the Vue frontends are set up for TypeScript: the three tsconfigs, the type check, and what stays JavaScript."
---

The frontends are Vue 3 applications built with Vite and tested with Vitest. The aim is TypeScript for code that runs in the browser. The editor, lawmaking, the demo and `frontend-shared` use the setup on this page; the PoC frontends are still JavaScript and will be migrated later. Three kinds of file stay JavaScript in any case: scripts that run in plain Node, the modules those scripts load, and generated files such as `grammar.generated.js` and `nldd-components.js`. In the demo, the first two are the modules in `src/i18n/` (the dictionaries, the language table and `hash.js`) and `src/data/materialize.js`, which the scripts in `scripts/` and `corpus/demo/tools/` load.

## The tsconfigs

An app follows the layout that `create-vue` generates for a TypeScript project. `tsconfig.json` references three programs, one per environment:

| Config | Checks | Environment |
|--------|--------|-------------|
| `tsconfig.app.json` | `src/` without the tests, and `env.d.ts` | the browser, through `@vue/tsconfig/tsconfig.dom.json` |
| `tsconfig.node.json` | `vite.config.ts`, `vitest.config.ts` | Node, through `@tsconfig/node24` |
| `tsconfig.vitest.json` | `src/**/*.test.ts` | Vitest |

`npm run typecheck` runs `vue-tsc --build` over all three, and CI runs it as a separate step in the frontend job. The app config sets `noUncheckedIndexedAccess`, so an `arr[i]` lookup is typed as possibly `undefined`, and `allowJs` where JavaScript modules remain. `vitest.config.ts` builds on `vite.config.ts`, so a test that imports a component gets the same template compiler options as the app. A test that mounts a component runs in happy-dom.

## Conventions

- Nothing is compiled ahead of time. Vite and Vitest strip the types when they bundle or test the source.
- An import of a TypeScript module leaves out the extension (`'./values'`). Vite and Vitest resolve it, and the code only runs through them. Plain Node does not resolve an import without an extension, so a module that a `node` script loads keeps its `.js` imports.
- The check runs `vue-tsc`, not `tsc`, because only `vue-tsc` reads the script blocks in `.vue` files. The workspace pins TypeScript 6 through an `overrides` entry in the root `package.json`: `vue-tsc` builds on the JavaScript compiler API that TypeScript 7 no longer ships. The docs site has its own lockfile and stays on TypeScript 7.

## Where tests go

A vitest test sits next to the module it tests, in every frontend and PoC: in `src/` for app code, in `scripts/` or `server/` for the Node code there. Test helpers go in `src/test-utils/`.

A PoC also has a `tests/` directory, for the tests that reach outside the app. They load the engine built to WASM, or read the casus corpus, the casus data or a fixture of the Rust crate from disk. The scenario runners `tests/scenarios.test.js` and their helpers in `tests/helpers/` are among them. Such a test exercises no single module, so it has nothing to sit next to.

`frontend/e2e/` holds the Playwright specs and their helpers. The corpus loader those specs share lives in `frontend/src/test-utils/`, where its vitest test runs.
