import { defineConfig } from 'vitest/config';

// Playwright in a real headless Chromium (`npx playwright-core install chromium-headless-shell`).
export default defineConfig({
  test: {
    environment: 'node',
    include: ['e2e/**/*.e2e.test.ts'],
    testTimeout: 60_000,
  },
});
