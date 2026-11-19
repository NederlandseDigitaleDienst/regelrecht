import { fileURLToPath } from 'url';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  resolve: {
    alias: { '@eerste': fileURLToPath(new URL('./src/eerste', import.meta.url)) },
  },
  test: {
    environment: 'node',
    include: ['tests/**/*.test.js'],
  },
});
