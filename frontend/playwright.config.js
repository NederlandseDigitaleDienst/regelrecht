import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  // Only the browser specs. The default pattern also picks up `*.test.js`,
  // the name of a vitest test; those belong in `src/`, next to the code.
  testMatch: '**/*.spec.js',
  timeout: 30_000,
  retries: process.env.CI ? 1 : 0,
  // Eén worker in CI. De suite leunt op vaste wachttijden in plaats van op
  // condities, en valt daardoor onder parallelle belasting om (full-roundtrip
  // "YAML textarea round-trips without data loss" is de bekende). Als
  // verplichte check moet hij deterministisch zijn; lokaal blijft de default
  // parallellisme, want daar is een herstart goedkoop.
  workers: process.env.CI ? 1 : undefined,
  use: {
    baseURL: 'http://localhost:7100',
    headless: true,
  },
  webServer: {
    command: 'npx vite --port 7100 --host 0.0.0.0',
    port: 7100,
    reuseExistingServer: !process.env.CI,
  },
  projects: [
    {
      name: 'chromium',
      use: { browserName: 'chromium' },
    },
  ],
});
