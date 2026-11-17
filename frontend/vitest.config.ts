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
      // `e2e/` holds Playwright specs (`*.spec.js`) plus plain Node helpers those
      // specs share; the helpers' own unit tests are `*.test.js` and run here.
      // Playwright's `testMatch` is narrowed to `*.spec.js` so the two never
      // collect each other's files.
      include: ['src/**/*.test.{js,ts}', 'e2e/**/*.test.js'],
      exclude: [...configDefaults.exclude],
      root: fileURLToPath(new URL('./', import.meta.url)),
      // Every test starts with an empty in-memory corpus reader and a fresh
      // Pinia installed.
      setupFiles: ['src/test-utils/corpusSetup.ts', 'src/test-utils/piniaSetup.ts'],
      pool: 'vmThreads',
      testTimeout: 10000,
      server: {
        // @cucumber/gherkin 41 and @cucumber/messages 34 ship as pure ESM. The
        // vmThreads pool loads external ESM in a separate VM context, which throws
        // "Linked modules must use the same context". Inlining lets vitest process
        // them in the test context instead. The @regelrecht/frontend-shared
        // workspace package (ESM) hits the same issue when a test transitively
        // imports it (e.g. usePollingFetch → apiFetch), so inline it too.
        deps: {
          inline: [/@cucumber\//, /@regelrecht\//],
        },
      },
    },
  }),
);
