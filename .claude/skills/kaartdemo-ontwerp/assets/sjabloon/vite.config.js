import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// Achter het poc-portaal staat de app onder /%%SLUG%%/; los draait hij op /.
const base = process.env.POC_BASE ?? '/';

export default defineConfig({
  base,
  plugins: [
    vue({
      template: { compilerOptions: { isCustomElement: (tag) => tag.startsWith('nldd-') } },
    }),
  ],
  // src/main.js wacht met top-level await op de gegevens.
  build: { target: 'es2022' },
  server: { port: %%POORT%% },
});
