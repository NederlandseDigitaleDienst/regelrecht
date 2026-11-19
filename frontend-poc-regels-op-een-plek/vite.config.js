import { fileURLToPath } from 'url';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { bronDev } from './scripts/bron-dev.js';

const base = process.env.POC_BASE ?? '/';

export default defineConfig({
  // Achter het poc-portaal staat deze app onder /regels-op-een-plek/; los draait hij op /.
  // De gegevens komen uit de bron (src/bron.js): achter het portaal serveert het portaal
  // die, hier de dev-server uit POC_BRON_SPOORZONE (scripts/bron-dev.js).
  base,
  resolve: {
    // De kaart, de toets en de simulatie zonder DSO staan in src/eerste.
    alias: { '@eerste': fileURLToPath(new URL('./src/eerste', import.meta.url)) },
  },
  plugins: [
    bronDev(base),
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
  ],
  // src/main.js wacht met top-level await op de bron.
  build: { target: 'es2022' },
  server: {
    port: 5420,
  },
});
