import { defineConfig } from 'vitest/config';

// The page script in a real headless Chromium (`npx playwright-core install
// chromium-headless-shell`): the computed style per breakpoint width, before and after a
// navigation.
export default defineConfig({
  test: {
    environment: 'node',
    include: ['e2e/**/*.e2e.test.ts'],
    testTimeout: 60_000,
  },
});
