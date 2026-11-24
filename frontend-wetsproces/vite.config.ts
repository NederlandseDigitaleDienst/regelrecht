import { rename } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import type { Plugin } from 'vite';
import vue from '@vitejs/plugin-vue';

// The server in packages/wetsproces serves the built app beside its API, on
// one origin, so the session cookie goes with every call. In development the
// dev server passes the API and the login through to that server
// (`just wetsproces`, port 7180 by default).
const api = process.env.WETSPROCES_API ?? 'http://localhost:7180';

// One source, four builds, chosen by mode. Each web build lands in
// dist/<surface>/ with an index.html, and the server serves the one of its
// WETSPROCES_SURFACE (packages/wetsproces/src/surface.rs):
// - `--mode all` (the default): the combined app (index.html), both sides,
//   for a server on `all` in test mode;
// - `--mode burger`: the citizen's entry (burger.html) with HTTP and the
//   session cookie, for the environment of citizens;
// - `--mode backoffice`: the back office's entry (backoffice.html), for the
//   environment of employees;
// - `--mode organisatie`: the front of an organisation (organisatie.html),
//   for a server in the role `organisatie`, on either side;
// - `--mode app`: the citizen's entry with the transport of the Tauri app,
//   into dist-app/, which the shell in src-tauri/ embeds as burger.html.
// The citizen's entry chooses its transport through the import `#vervoer`
// (src/burgeromgeving/vervoer.ts), so a bundle holds only its own.
interface Build {
  entry: string;
  outDir: string;
  vervoer?: string;
  /** Write the entry as index.html, where the server looks for it. */
  index: boolean;
}
const builds: Record<string, Build> = {
  all: { entry: 'index.html', outDir: 'dist/all', index: false },
  burger: { entry: 'burger.html', outDir: 'dist/burger', vervoer: 'cookie', index: true },
  backoffice: { entry: 'backoffice.html', outDir: 'dist/backoffice', index: true },
  organisatie: { entry: 'organisatie.html', outDir: 'dist/organisatie', index: true },
  app: { entry: 'burger.html', outDir: 'dist-app', vervoer: 'tauri', index: false },
};

/**
 * Rename the written page `entry` to index.html. On disk, after the bundle is
 * written: the bundler does not let a plugin rename an output in the bundle.
 */
function alsIndex(entry: string): Plugin {
  return {
    name: 'wetsproces-als-index',
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
    ],
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
      port: 7181,
      proxy: {
        '/api': api,
        '/auth': api,
      },
    },
  };
});
