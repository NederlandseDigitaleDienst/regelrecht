// Generate .pa11yci from the built site so the accessibility gate covers
// every page, not a hand-picked sample. Run after `astro build`: it globs
// dist/ for every route (each dir with an index.html, plus top-level *.html
// like 404.html) and writes the pa11y-ci config.
//
// Why generate instead of hand-list: docs pages are added often, and a static
// list silently stops covering new pages. Globbing dist guarantees the gate
// tests exactly what ships.
//
// Runners: htmlcs + axe. htmlcs (HTML_CodeSniffer) covers the classic WCAG AA
// set including its own contrast algorithm. axe-core (4.11.x, resolved by
// pa11y's axe runner from the top-level dependency) adds a second engine with
// a modern contrast algorithm — the historical 4.2 false-positives on the
// landing hero/step cards are gone, verified by running the gate. Two engines
// catch more than one; a finding in either fails the gate.

import { readdirSync, statSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const DOCS_DIR = fileURLToPath(new URL('..', import.meta.url));
const DIST = join(DOCS_DIR, 'dist');
const ORIGIN = 'http://127.0.0.1:4173';

// Walk dist for every index.html (→ a route) and every top-level *.html.
function collectRoutes(dir) {
  const routes = new Set();
  function walk(current) {
    for (const entry of readdirSync(current)) {
      const full = join(current, entry);
      if (statSync(full).isDirectory()) {
        walk(full);
      } else if (entry === 'index.html') {
        const rel = relative(DIST, current).split('\\').join('/');
        routes.add(rel === '' ? '/' : `/${rel}`);
      } else if (entry.endsWith('.html')) {
        // Top-level pages without their own directory, e.g. 404.html.
        const rel = relative(DIST, full).split('\\').join('/');
        routes.add(`/${rel.replace(/\.html$/, '')}`);
      }
    }
  }
  walk(DIST);
  return [...routes].sort();
}

// Beperk de gate tot de pagina's die een PR daadwerkelijk raakt, als dat veilig
// kan. Alleen inhoudsbestanden zijn veilig: een layout, component, stylesheet of
// configuratie verandert de uitvoer van elke pagina. Twijfel valt altijd terug
// op de volledige set — een gate die te weinig controleert is erger dan een
// trage gate.
const CONTENT_FILE = /^docs\/src\/content\/(docs|rfcs)\/(.+)\.mdx?$/;

function scopeRoutes(allRoutes) {
  const raw = (process.env.A11Y_CHANGED_FILES ?? '').trim();
  if (raw === '') return null;

  const changed = raw.split(/\s+/).filter(Boolean);
  const selected = new Set();

  for (const file of changed) {
    const match = file.match(CONTENT_FILE);
    if (!match) {
      console.log(`Volledige gate: ${file} is geen inhoudsbestand.`);
      return null;
    }
    const [, collection, rest] = match;
    // Een kale `index` hoort ook weg, niet alleen `…/index`: src/content/
    // rfcs/index.md is de route /rfcs, niet /rfcs/index.
    const slug = rest.replace(/(^|\/)index$/, '');
    const route =
      collection === 'rfcs'
        ? slug === ''
          ? '/rfcs'
          : `/rfcs/${slug}`
        : slug === ''
          ? '/docs'
          : `/${slug}`;
    if (!allRoutes.includes(route)) {
      console.log(`Volledige gate: ${file} kon niet op een route worden afgebeeld (${route}).`);
      return null;
    }
    selected.add(route);

    // De indexpagina's erboven sommen deze pagina op, dus die verandert mee.
    const parts = route.split('/').filter(Boolean);
    for (let i = parts.length - 1; i > 0; i -= 1) {
      const ancestor = `/${parts.slice(0, i).join('/')}`;
      if (allRoutes.includes(ancestor)) selected.add(ancestor);
    }
    if (allRoutes.includes('/')) selected.add('/');
  }

  return [...selected].sort();
}

// A11Y_SHARD=K/N keeps every N-th selected route, starting at the K-th, so CI
// can split the gate over N jobs that each start their own Chromium (sharing
// one browser is what `concurrency` broke, see below). Unset means 1/1. A
// malformed value stops the script: silently checking every page, or none,
// would hide that the split is wrong.
function parseShard(raw) {
  if (raw === undefined || raw.trim() === '') return [1, 1];
  const match = /^(\d+)\/(\d+)$/.exec(raw.trim());
  const [k, n] = match ? [Number(match[1]), Number(match[2])] : [NaN, NaN];
  if (!(Number.isInteger(k) && Number.isInteger(n) && k >= 1 && k <= n)) {
    console.error(`A11Y_SHARD expects K/N with 1 <= K <= N, got ${JSON.stringify(raw)}.`);
    process.exit(1);
  }
  return [k, n];
}

const [shardK, shardN] = parseShard(process.env.A11Y_SHARD);

const routes = collectRoutes(DIST);
if (routes.length === 0) {
  console.error('No routes found in dist/. Run `astro build` first.');
  process.exit(1);
}

const scoped = scopeRoutes(routes);
const selected = scoped ?? routes;
const checked = selected.filter((_, i) => i % shardN === shardK - 1);

const config = {
  defaults: {
    // Geen `concurrency` hier. Het scheelt fors (261 → 143 seconden op 89
    // pagina's), maar pa11y-ci deelt dan één Chromium en struikelt over
    // `Target.closeTarget`: 54 van de 89 pagina's faalden op die protocolfout
    // in plaats van op een bevinding. Een wisselvallige gate is erger dan een
    // trage; de winst zit in minder pagina's controleren, niet in ze
    // tegelijk controleren.
    standard: 'WCAG2AA',
    runners: ['htmlcs', 'axe'],
    timeout: 30000,
    wait: 500,
    chromeLaunchConfig: { args: ['--no-sandbox'] },
    // axe reports two flavours of finding: confirmed `violations` and
    // `incomplete` (a.k.a. `needsFurtherReview`) where it could not measure
    // the rule reliably and is asking for manual review. Pa11y promotes both
    // to errors by default, which fails CI on the latter category — for
    // color-contrast that mostly happens on slotted content inside design-
    // system custom elements whose effective background axe can't sample
    // through the shadow boundary (e.g. nldd-rich-text inside an
    // nldd-*-section: the same paragraph type passes on some pages and
    // "fails" on others depending on stacking context). Capping incomplete
    // items at `warning` keeps real violations as errors while letting
    // axe-can't-tell items show up as non-blocking warnings (visible via
    // `pa11y --include-warnings` for spot checks).
    levelCapWhenNeedsReview: 'warning',
    // hideElements removes these subtrees from BOTH runners' evaluation.
    //  - .pagefind-ui: search UI is injected client-side and is the design
    //    system's concern, not this site's markup.
    //  - svg[id^="mermaid-"]: mermaid renders diagrams as inline SVG whose text
    //    lives in <foreignObject> over transparent layers, and whose shapes
    //    carry inline fill="…" attributes. axe's color-contrast algorithm can't
    //    resolve the effective background through that structure and flags every
    //    label, even though the rendered contrast is fine (state/flowchart text
    //    is donkerblauw-900 on lichtblauw-50 ≈ 14.4:1; C4 keeps its native white
    //    on dark blue, well above the requirement — both measured in the
    //    browser). The
    //    diagrams' accessible name (role=img + aria-label) is asserted by
    //    scripts/check-mermaid-alt.mjs, which the a11y npm script runs against
    //    the build before this gate, so hiding the SVG here does not lose that
    //    check. Hiding the SVG keeps the gate honest about everything else.
    //  - pre.astro-code: Shiki emits dual themes as one element
    //    (color:<light>;--shiki-dark:<dark>, same for bg). axe evaluates the
    //    static light color against a background it samples inconsistently and
    //    reports phantom contrast failures on box-drawing/whitespace tokens,
    //    while the actually-rendered text clears the 4.5:1 AA requirement in
    //    both themes. The high-contrast GitHub themes are deliberately chosen
    //    for this; the gate can't see it.
    hideElements:
      '.pagefind-ui, svg[id^="mermaid-"], pre.astro-code',
  },
  '//generated': 'Generated by scripts/gen-pa11yci.mjs from dist/. Do not edit by hand; run `npm run a11y` (or `just docs-a11y`) which regenerates this.',
  urls: checked.map((r) => `${ORIGIN}${r === '/' ? '/' : r}`),
};

const out = join(DOCS_DIR, '.pa11yci');
writeFileSync(out, JSON.stringify(config, null, 2) + '\n');
const shardNote = shardN > 1 ? `, shard ${shardK}/${shardN} of ${selected.length}` : '';
console.log(
  scoped
    ? `Wrote ${out} with ${checked.length} of ${routes.length} routes (beperkt tot de gewijzigde pagina's${shardNote}; runners: htmlcs + axe).`
    : `Wrote ${out} with ${checked.length} routes (volledige site${shardNote}; runners: htmlcs + axe).`,
);
