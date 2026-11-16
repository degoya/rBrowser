import { defineConfig } from 'vitest/config';

// The page script against ../tests/fixtures/*.html in a real headless Chromium (Playwright's
// chromium-headless-shell: `npx playwright-core install chromium-headless-shell`). Needs
// `npm run build` first: it loads the built ../page/simulators.js.
export default defineConfig({
  test: {
    environment: 'node',
    include: ['e2e/**/*.e2e.test.ts'],
    testTimeout: 60_000,
    hookTimeout: 60_000,
  },
});
