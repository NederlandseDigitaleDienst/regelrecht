import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// The server in packages/wetsproces serves the built app beside its API, on
// one origin, so the session cookie goes with every call. In development the
// dev server passes the API and the login through to that server
// (`just wetsproces`, port 7180 by default).
const api = process.env.WETSPROCES_API ?? 'http://localhost:7180';

export default defineConfig({
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag: string) => tag.startsWith('nldd-'),
        },
      },
    }),
  ],
  build: {
    cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari18'],
    outDir: 'dist',
  },
  server: {
    host: '0.0.0.0',
    port: 7181,
    proxy: {
      '/api': api,
      '/auth': api,
    },
  },
});
