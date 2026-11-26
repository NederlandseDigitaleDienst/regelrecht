import { createReadStream, existsSync } from 'node:fs';
import { cp, rename } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import type { Plugin } from 'vite';
import vue from '@vitejs/plugin-vue';

// The server in packages/regelhet serves the built app beside its API, on
// one origin, so the session cookie goes with every call. In development the
// dev server passes the API and the login through to that server
// (`just regelhet`, port 7180 by default).
const api = process.env.REGELHET_API ?? 'http://localhost:7180';

// One source, four builds, chosen by mode. Each web build lands in
// dist/<surface>/ with an index.html, and the server serves the one of its
// REGELHET_SURFACE (packages/regelhet/src/surface.rs):
// - `--mode all` (the default): the combined app (index.html), both sides,
//   for a server on `all` in test mode;
// - `--mode burger`: the citizen's entry (burger.html) with HTTP and the
//   session cookie, for the environment of citizens;
// - `--mode backoffice`: the back office's entry (backoffice.html), for the
//   environment of employees;
// - `--mode organisatie`: the front of an organisation (organisatie.html),
//   for a server in the role `organisatie`, on either side;
// - `--mode app`: the citizen's entry with the transport of the Tauri app,
//   into dist-app/, which the shell in src-tauri/ embeds as burger.html;
// - `--mode burgerapp`: the citizen's entry as their own app in the browser,
//   over FSC voor Burgers to the RegelHets of its list, into dist-burgerapp/:
//   a static site, with the kern as WebAssembly and the list beside it
//   (`burgerappBestanden`).
// The citizen's entry chooses its transport through the import `#vervoer`
// (src/burgeromgeving/vervoer.ts), so a bundle holds only its own.
interface Build {
  entry: string;
  outDir: string;
  vervoer?: string;
  /** Write the entry as index.html, where the server looks for it. */
  index: boolean;
  /** Serve and ship the kern as WASM and the list of RegelHets beside the app. */
  burgerapp?: boolean;
}
const builds: Record<string, Build> = {
  all: { entry: 'index.html', outDir: 'dist/all', index: false },
  burger: { entry: 'burger.html', outDir: 'dist/burger', vervoer: 'cookie', index: true },
  backoffice: { entry: 'backoffice.html', outDir: 'dist/backoffice', index: true },
  organisatie: { entry: 'organisatie.html', outDir: 'dist/organisatie', index: true },
  app: { entry: 'burger.html', outDir: 'dist-app', vervoer: 'tauri', index: false },
  burgerapp: { entry: 'burger.html', outDir: 'dist-burgerapp', vervoer: 'fsc4b', index: true, burgerapp: true },
};

// What the citizen's app in the browser loads beside its bundle, by its path
// in the site: the kern as WASM, from `just burgerapp-wasm` (or the WASM stage
// of burgerapp/Dockerfile), and the list of RegelHets, which the deployment
// replaces with its own.
const wasmPkg = process.env.BURGERAPP_WASM_PKG ?? fileURLToPath(new URL('./burgerapp/wasm/pkg', import.meta.url));
const burgerappStatisch: Record<string, string> = {
  'wasm/pkg': wasmPkg,
  'regelhets.json': fileURLToPath(new URL('./burgerapp/regelhets.json', import.meta.url)),
};

/**
 * Serve the files of `burgerappStatisch` in development and copy them into
 * the build. The WASM is built from Rust and not by Vite; a build without it
 * stops here, rather than shipping an app that says so only in the browser.
 */
function burgerappBestanden(): Plugin {
  const ontbreekt = () =>
    Object.entries(burgerappStatisch).find(([, van]) => !existsSync(van))?.[1];
  return {
    name: 'regelhet-burgerapp-bestanden',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        let pad: string;
        try {
          pad = decodeURIComponent((req.url ?? '').split('?')[0] ?? '').replace(/^\//, '');
        } catch {
          return next();
        }
        for (const [onder, van] of Object.entries(burgerappStatisch)) {
          const rest = pad === onder ? '' : pad.startsWith(`${onder}/`) ? pad.slice(onder.length + 1) : null;
          if (rest === null || rest.includes('..')) continue;
          const bestand = rest ? join(van, rest) : van;
          // A file that is not there is a 404, as nginx answers it, and not
          // the page of the app.
          if (!existsSync(bestand)) {
            res.statusCode = 404;
            res.end();
            return;
          }
          if (bestand.endsWith('.wasm')) res.setHeader('content-type', 'application/wasm');
          createReadStream(bestand).pipe(res);
          return;
        }
        next();
      });
    },
    async writeBundle(options) {
      const mist = ontbreekt();
      if (mist) throw new Error(`${mist} is er niet; bouw de rekenkern eerst met \`just burgerapp-wasm\``);
      const dir = options.dir ?? 'dist-burgerapp';
      for (const [onder, van] of Object.entries(burgerappStatisch)) await cp(van, join(dir, onder), { recursive: true });
    },
  };
}

/**
 * Rename the written page `entry` to index.html. On disk, after the bundle is
 * written: the bundler does not let a plugin rename an output in the bundle.
 */
function alsIndex(entry: string): Plugin {
  return {
    name: 'regelhet-als-index',
    apply: 'build',
    async writeBundle(options) {
      const dir = options.dir ?? 'dist';
      await rename(join(dir, entry), join(dir, 'index.html'));
    },
  };
}

export default defineConfig(({ mode }) => {
  const build = builds[mode];
  // Tests and the type check see the app's transport.
  const vervoer = build?.vervoer ?? 'tauri';
  return {
    plugins: [
      vue({
        template: {
          compilerOptions: {
            isCustomElement: (tag: string) => tag.startsWith('nldd-'),
          },
        },
      }),
      ...(build?.index ? [alsIndex(build.entry)] : []),
      ...(build?.burgerapp ? [burgerappBestanden()] : []),
    ],
    // The citizen's app is a static site that may live under any path.
    base: build?.burgerapp ? './' : '/',
    resolve: {
      alias: {
        '#vervoer': fileURLToPath(new URL(`./src/burgeromgeving/vervoer/${vervoer}.ts`, import.meta.url)),
      },
    },
    build: {
      cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari18'],
      outDir: build?.outDir ?? builds.all?.outDir,
      rollupOptions: { input: build?.entry ?? 'index.html' },
    },
    server: {
      host: '0.0.0.0',
      // The citizen's app has a port of its own: a RegelHet allows its origin
      // by name (REGELHET_FSC4B_ORIGINS), so it should not move.
      port: build?.burgerapp ? 7150 : 7181,
      proxy: {
        '/api': api,
        '/auth': api,
      },
    },
  };
});
