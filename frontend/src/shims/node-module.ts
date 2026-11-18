// Browser shim for Node's `node:module`.
//
// @cucumber/messages 33 (pure ESM) reads its own version at import time in
// dist/version.js:
//
//   import { createRequire } from 'node:module';
//   export const version = createRequire(import.meta.url)('../package.json').version;
//
// `createRequire` is a Node-only API; in the browser the bundler stubs
// `node:module` to an empty object, so `createRequire` is undefined and the
// call throws `TypeError: createRequire is not a function` at module load -
// crashing every view that imports the gherkin parser (the whole editor).
//
// We never use that version string, so provide a minimal `createRequire` whose
// returned require() yields an object with a `version` field. This alias is
// applied to the browser build only (see vite.config.js); Node-based vitest
// keeps the real `node:module`.
export function createRequire() {
  return () => ({ version: '0.0.0' });
}

export default { createRequire };
