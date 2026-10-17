import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
  // Achter het poc-portaal staat deze app onder /vakantieverlof/; los draait
  // hij op /. De backend serveert zichzelf onder hetzelfde voorvoegsel
  // (VERLOF_BASE_PATH), zodat het portaal het pad ongewijzigd doorstuurt.
  base: process.env.POC_BASE ?? '/',
  plugins: [
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
    proxy: {
      // packages/poc-vakantieverlof, standaard op 8410.
      '/api': 'http://localhost:8410',
    },
  },
});
