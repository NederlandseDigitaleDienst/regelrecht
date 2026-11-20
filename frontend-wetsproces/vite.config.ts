import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// The server in packages/wetsproces serves the built app beside its API, on
// one origin, so the session cookie goes with every call. In development the
// dev server passes the API and the login through to that server
// (`just wetsproces`, port 7180 by default).
const api = process.env.WETSPROCES_API ?? 'http://localhost:7180';

// The citizen's entry (burger.html, src/burgeromgeving/) holds the citizen's
// screens on their own, with one transport chosen here, by mode, through the
// import `#vervoer` (src/burgeromgeving/vervoer.ts):
// - `--mode app`: the transport of the Tauri app, into dist-app/, which the
//   shell in src-tauri/ embeds;
// - `--mode burger`: HTTP with the session cookie, into dist-burger/, for a
//   web environment of citizens only.
// Any other mode builds the combined web app (index.html) into dist/.
const burgerBuilds: Record<string, { vervoer: string; outDir: string }> = {
  app: { vervoer: 'tauri', outDir: 'dist-app' },
  burger: { vervoer: 'cookie', outDir: 'dist-burger' },
};

export default defineConfig(({ mode }) => {
  const burger = burgerBuilds[mode];
  // Tests and the type check see the app's transport.
  const vervoer = burger?.vervoer ?? 'tauri';
  return {
    plugins: [
      vue({
        template: {
          compilerOptions: {
            isCustomElement: (tag: string) => tag.startsWith('nldd-'),
          },
        },
      }),
    ],
    resolve: {
      alias: {
        '#vervoer': fileURLToPath(new URL(`./src/burgeromgeving/vervoer/${vervoer}.ts`, import.meta.url)),
      },
    },
    build: {
      cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari18'],
      ...(burger ? { outDir: burger.outDir, rollupOptions: { input: 'burger.html' } } : { outDir: 'dist' }),
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
