// Check that links from this site into the demo land on the right page, in
// the right language.
//
// The demo translates its paths: the simulation is `/simulatie` in Dutch and
// `/en/simulation` in English. `src/lib/demo-links.js` holds a copy of those
// paths so a page can link to the demo in its own language. Two things can go
// wrong with a copy, and both stay silent in the browser: the demo's router
// redirects an unknown path to its home page instead of showing an error.
//
// 1. The copy drifts. A route renamed in `frontend-demo/src/router.ts` leaves
//    the old path here, and every link to it lands on the demo's home page.
// 2. A page bypasses the copy with a hardcoded address. That is how the
//    English landing came to link to the Dutch demo.
//
// Reads source files, not the build, so it runs without `astro build`.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { DEMO_ORIGIN, DEMO_PATHS } from '../src/lib/demo-links.js';

const ROUTER = fileURLToPath(
  new URL('../../frontend-demo/src/router.ts', import.meta.url),
);
const SRC_DIR = fileURLToPath(new URL('../src', import.meta.url));
const TABLE = 'src/lib/demo-links.js';
const LANGS = ['nl', 'en'];

/**
 * The demo's paths per page and language, read from its route table.
 *
 * An optional route parameter is not part of the address a link names
 * (`/rondleiding` for `/rondleiding/:faqId?`), so it is dropped. A required
 * one stays, and then no path in the table can match: a page that needs a
 * parameter cannot be linked through this table as it is.
 */
function demoRoutes() {
  const text = readFileSync(ROUTER, 'utf8');
  const routes = new Map();
  for (const m of text.matchAll(/name:\s*'([\w-]+)',\s*paths:\s*\{([^}]*)\}/g)) {
    const paths = {};
    for (const p of m[2].matchAll(/(\w+):\s*'([^']*)'/g)) {
      paths[p[1]] = p[2].replace(/(\/:\w+(\([^)]*\))?\?)+$/, '') || '/';
    }
    routes.set(m[1], paths);
  }
  return routes;
}

function sourceFiles(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = `${dir}/${name}`;
    return statSync(path).isDirectory() ? sourceFiles(path) : [path];
  });
}

const errors = [];
const routes = demoRoutes();
if (routes.size === 0) {
  errors.push(`no routes found in ${ROUTER}; has the route table changed shape?`);
}

for (const [page, paths] of Object.entries(DEMO_PATHS)) {
  const route = routes.get(page);
  if (!route) {
    errors.push(`${TABLE}: "${page}" is not a page in the demo's route table`);
    continue;
  }
  for (const lang of LANGS) {
    if (paths[lang] !== route[lang]) {
      errors.push(
        `${TABLE}: "${page}" in ${lang} is "${paths[lang]}", the demo has "${route[lang]}"`,
      );
    }
  }
}

// An address in backticks or prose without a scheme is a name, not a link;
// only a full URL can send a reader somewhere. `DEMO_ORIGIN` outside the table
// is the same shortcut with one more step: an address built by hand.
const hardcoded = new RegExp(`https?://${DEMO_ORIGIN.replace(/^https?:\/\//, '').replace(/\./g, '\\.')}`);
for (const file of sourceFiles(SRC_DIR)) {
  if (file.endsWith(TABLE.replace('src/', ''))) continue;
  if (!/\.(astro|ts|js|mjs|md|mdx|json)$/.test(file)) continue;
  readFileSync(file, 'utf8')
    .split('\n')
    .forEach((line, i) => {
      if (hardcoded.test(line) || /\bDEMO_ORIGIN\b/.test(line)) {
        errors.push(
          `${file.slice(SRC_DIR.length - 3)}:${i + 1}: hardcoded demo address; use demoUrl() from ${TABLE}`,
        );
      }
    });
}

if (errors.length > 0) {
  console.error('check-demo-links: links into the demo are not in order\n');
  for (const e of errors) console.error(`  ${e}`);
  process.exit(1);
}
console.log(
  `check-demo-links: ${Object.keys(DEMO_PATHS).length} demo pages match the demo's route table, no hardcoded addresses`,
);
