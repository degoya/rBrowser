import { defineConfig } from 'vitest/config';

// The tests read the fixture pages of `tests/fixtures/site/` (`?raw`), outside this package.
export default defineConfig({
  server: { fs: { allow: ['..'] } },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts'],
  },
});
