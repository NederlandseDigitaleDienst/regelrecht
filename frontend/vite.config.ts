import { readFileSync } from 'node:fs';
import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// @cucumber/messages 34 runs `createRequire(import.meta.url)('../package.json')`
// at import time (a Node-only API). In the browser build that throws and
// crashes every view importing the gherkin parser, so alias `node:module` to a
// browser shim that provides a harmless `createRequire`. Scope it to the build
// only - under vitest (Node) the real `node:module` works, so we leave it.
const isVitest = !!process.env.VITEST;
const nodeModuleShim = fileURLToPath(
  new URL('./src/shims/node-module.ts', import.meta.url),
);

// Backend port the dev proxy forwards /api, /auth and /health to. Defaults to
// 8000 (editor-api); `just dev` sets API_PORT so multiple backends can
// coexist on distinct ports.
const apiTarget = `http://localhost:${process.env.API_PORT || '8000'}`;

// The laws the search popover suggests to someone not logged in. Read at build
// time from public/favorites.json, the same list the editor-api loads into
// Corpus juris at startup, so a suggestion always resolves there.
const suggestedLawIds: string[] = JSON.parse(
  readFileSync(fileURLToPath(new URL('./public/favorites.json', import.meta.url)), 'utf8'),
);

export default defineConfig({
  root: '.',
  define: {
    __SUGGESTED_LAW_IDS__: JSON.stringify(suggestedLawIds),
  },
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag: string) => tag.startsWith('nldd-'),
        },
      },
    }),
    {
      name: 'spa-fallback',
      configureServer(server) {
        server.middlewares.use((req, _res, next) => {
          const url = (req.url ?? '').split('?')[0] ?? '';
          if (
            url === '/' ||
            url === '/editor.html' ||
            (url.startsWith('/library') && !url.includes('.')) ||
            (url.startsWith('/editor') && !url.includes('.'))
          ) {
            req.url = '/index.html';
          }
          next();
        });
      },
    },
  ],
  resolve: {
    alias: isVitest ? {} : { 'node:module': nodeModuleShim },
  },
  build: {
    cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari18'],
    outDir: 'dist',
    rolldownOptions: {
      output: {
        // Without this group, rolldown parks its shared module-namespace
        // runtime helper inside the OverviewView chunk (the only chunk that
        // needs CJS interop, via echarts). Several design-system chunks in the
        // static entry graph import that helper, which drags the ~550 KB
        // echarts payload into index.html's modulepreload list even though the
        // route itself is lazy. Grouping echarts into its own chunk makes
        // rolldown emit a separate tiny runtime chunk, and echarts is then
        // only fetched when a harvester view actually opens.
        //
        // `includeDependenciesRecursively: false` is required: the default
        // (true) pulls the captured modules' dependencies (Vue's runtime-core,
        // reactivity, tslib) into the echarts chunk, which puts it right back
        // in the entry graph.
        //
        // `scripts/check-first-load.mjs` (postbuild) guards this: if the
        // option stops working the build fails instead of echarts silently
        // rejoining the first load.
        codeSplitting: {
          groups: [
            {
              name: 'echarts',
              test: /node_modules[\\/](echarts|zrender|vue-echarts|tslib)[\\/]/,
              includeDependenciesRecursively: false,
            },
          ],
        },
      },
    },
  },
  server: {
    port: 3000,
    proxy: {
      '/api': apiTarget,
      '/auth': apiTarget,
      '/health': apiTarget,
    },
  },
});
