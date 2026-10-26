import { fileURLToPath } from 'node:url';
import { mergeConfig, defineConfig, configDefaults } from 'vitest/config';
import viteConfig from './vite.config';

// Built on the Vite config, so a component test compiles templates the way the
// app does (the `nldd-` custom elements included).
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'happy-dom',
      // `server/` is the "why" backend, plain Node; its test stays JavaScript.
      include: ['src/**/*.test.{js,ts}', 'server/**/*.test.js'],
      exclude: [...configDefaults.exclude],
      root: fileURLToPath(new URL('./', import.meta.url)),
      server: {
        deps: {
          inline: [/@cucumber\//, /@regelrecht\//],
        },
      },
    },
  }),
);
