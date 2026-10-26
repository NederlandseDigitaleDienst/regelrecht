import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import pagefind from 'astro-pagefind';
import remarkGfm from 'remark-gfm';
import rehypeMermaid from 'rehype-mermaid';
import { rehypeMermaidAlt } from './src/lib/rehype-mermaid-alt.ts';
import { rehypeNlddCodeViewer } from './src/lib/rehype-nldd-code-viewer.ts';
import { rehypeSourceLines } from './src/lib/rehype-source-lines.ts';
import { rehypeRfcLinks } from './src/lib/rehype-rfc-links.ts';
import { rehypeIssueLinks } from './src/lib/rehype-issue-links.ts';

export default defineConfig({
  site: 'https://docs.regelrecht.rijks.app',
  // `format: 'directory'` emits every page as `<route>/index.html`, so the
  // index routes the internal links use with a trailing slash (`/en/`,
  // `/rfcs/`) resolve, and slash-less doc links (`/guide/what-is-regelrecht`)
  // are served directly by nginx trying `$uri/index.html` before the bare
  // directory — no 301 hop (see docs/nginx.conf try_files). `trailingSlash:
  // 'ignore'` lets both `/x` and `/x/` work, matching that nginx rule.
  trailingSlash: 'ignore',
  build: {
    format: 'directory',
  },
  // Pagina's die van naam zijn veranderd houden hun oude adres. Een
  // documentatiepagina is een adres dat elders geciteerd wordt: in een RFC, in
  // een issue, in een bladwijzer, en dat adres stilletjes laten verdwijnen
  // verplaatst het probleem naar de lezer.
  //
  // `/concepts/untranslatables` heette zo toen het schemaveld nog
  // `untranslatables` heette. Schema v0.7.0 vervangt dat door `markings`
  // (RFC-031), dus de pagina heet nu naar wat ze beschrijft. De oude naam
  // blijft doorverwijzen, en de pagina zelf legt de hernoeming uit zodat
  // zoeken op de oude term er nog steeds uitkomt.
  redirects: {
    '/concepts/untranslatables': '/concepts/markings',
    // Het CJIB-pilotvoorstel is verwijderd: een ongevraagd voorstel aan een
    // andere organisatie, met bemensing, een tijdpad en aannames over hun
    // systemen, hoort niet op een publieke documentatiesite. De techniek die
    // erin stond (chronolexogram-types, de afleiding van de rechtsmiddel-route,
    // het `blauwe_knop`-blok) staat in RFC-022 en blijft daar staan.
    '/concepts/cjib-blauwe-knop-source-proposal': '/rfcs/rfc-022',
    // De open punten van chronolexografie zijn een deelpagina geworden en
    // heten nu open questions.
    '/concepts/chronolexography-open-points': '/concepts/chronolexography/open-questions',
    // De twee methodepagina's zijn één pagina geworden, en Multi-Org
    // Execution beschreef alleen een voorstel (RFC-009) dat niet gebouwd is;
    // wat ervan klopt staat nu bij Competent Authority.
    '/concepts/validation-methodology': '/concepts/methodology',
    '/concepts/multi-org-execution': '/concepts/competent-authority',
  },
  integrations: [
    mdx(),
    // force_language: en builds ONE index for the whole site instead of
    // splitting per <html lang>. The docs are English (57 pages); the
    // bilingual landing is the only Dutch page. Without this, Pagefind makes
    // a separate near-empty `nl` index and a search FROM the Dutch landing
    // never finds the docs (it only queries the index matching the page's
    // lang). One forced-English index makes all content searchable from
    // every page. (Verified at runtime: there is no client-side override —
    // init('en')/mergeIndex/options.language do not switch the index.)
    pagefind({ indexConfig: { forceLanguage: 'en' } }),
  ],
  markdown: {
    // Setting markdown.remarkPlugins / rehypePlugins (below) routes Markdown
    // through the `unified` processor from `@astrojs/markdown-remark`. Since
    // Astro 7.2 — Sätteri is the default Markdown processor — Astro no longer
    // installs that package itself, so docs/package.json depends on it
    // explicitly. Do not drop it again as "a peer of astro/mdx anyway": the
    // build hard-errors on this config without it.
    // No Shiki: rehype-nldd-code-viewer turns every fenced block into <nldd-code-viewer>,
    // which owns styling + (Prism) highlighting. Disabling Shiki also leaves
    // ```mermaid blocks as real <pre><code class="language-mermaid"> for
    // rehype-mermaid (it skips them via the language check).
    syntaxHighlight: false,
    // GFM tables (and the rest of GitHub-flavored markdown) parse into real
    // <table> nodes that nldd-rich-text then styles. Astro enables gfm for
    // plain .md by default, but @astrojs/mdx does not pick it up, so without
    // this an MDX page renders a pipe table as raw text. Listing remark-gfm
    // explicitly here covers both .md and .mdx (MDX extends markdown config).
    remarkPlugins: [remarkGfm],
    // inline-svg renders the diagram as an inline <svg> in the DOM (not an
    // <img>), so its colours can be themed with CSS and follow the .dark
    // toggle. Mermaid's `base` theme with neutral themeVariables hands the
    // colouring to docs.css (.mermaid svg rules), keyed off currentColor and
    // NLDD tokens, light and dark.
    rehypePlugins: [
      // Stamp source-line data attributes first, before the plugins below
      // restructure nodes and lose the original markdown positions.
      rehypeSourceLines,
      // Auto-link bare cross-references: RFC's ("RFC-008", "RFC-001 §9") and
      // issues/PR's ("issue #444", "PR #748"). Both run after source-lines
      // (they insert <a> nodes, which would otherwise perturb the line
      // stamping) and before the code-viewer reshape; both skip
      // <a>/<code>/<pre> so existing links and code examples are left alone.
      rehypeRfcLinks,
      rehypeIssueLinks,
      [
        rehypeMermaid,
        {
          strategy: 'inline-svg',
          mermaidConfig: {
            theme: 'base',
            themeVariables: {
              fontFamily: 'RijksSans, system-ui, sans-serif',
              // Transparent plate so the page background (light/dark) shows
              // through; nodes/edges/text are set in docs.css.
              background: 'transparent',
            },
          },
        },
      ],
      rehypeMermaidAlt,
      rehypeNlddCodeViewer,
    ],
  },
  vite: {
    ssr: {
      noExternal: ['@nldd/design-system'],
    },
  },
});
