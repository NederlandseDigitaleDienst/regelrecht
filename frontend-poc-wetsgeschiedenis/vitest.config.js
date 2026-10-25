import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';

// Een eigen bestand: vitest 4 en later lezen het `test`-blok in vite.config.js niet.
export default defineConfig({
  plugins: [
    vue({
      template: { compilerOptions: { isCustomElement: (tag) => tag.startsWith('nldd-') } },
    }),
  ],
  test: {
    environment: 'happy-dom',
    include: ['tests/**/*.test.js'],
  },
});
