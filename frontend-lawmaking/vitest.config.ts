import { fileURLToPath } from 'node:url';
import { mergeConfig, defineConfig, configDefaults } from 'vitest/config';
import viteConfig from './vite.config';

// Built on the Vite config, so a test that imports a component gets the same
// template compiler options as the app (the `nldd-` custom elements included).
// The tests run in Node; a component test that mounts will need a DOM
// environment such as happy-dom, as frontend-shared uses.
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      include: ['src/**/*.test.ts'],
      exclude: [...configDefaults.exclude],
      root: fileURLToPath(new URL('./', import.meta.url)),
    },
  }),
);
