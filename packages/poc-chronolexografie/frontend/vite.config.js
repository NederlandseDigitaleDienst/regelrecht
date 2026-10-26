import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { bronnenDev } from './scripts/bron-dev.js';

const base = process.env.POC_BASE ?? '/';

export default defineConfig({
  // Achter het poc-portaal staat deze app onder /chronolexografie/; los draait
  // hij op /. Er is geen backend: het spel draait als WASM in de browser
  // (src/api.js). Alleen de bronnen (bron/<naam>) komen van een server: het
  // portaal, of hier de dev-server uit POC_BRON_<NAAM> (scripts/bron-dev.js).
  base,
  plugins: [
    bronnenDev(base),
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('nldd-'),
        },
      },
    }),
  ],
  server: {
    port: 5410,
  },
});
