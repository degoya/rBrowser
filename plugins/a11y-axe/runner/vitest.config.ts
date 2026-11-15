import { defineConfig } from 'vitest/config';

// The tests read the fixture page `tests/fixtures/site/a11y.html` (`?raw`), outside this package.
export default defineConfig({
  server: { fs: { allow: ['../../..'] } },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts'],
  },
});
