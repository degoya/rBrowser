import { defineConfig } from 'vitest/config';

// The tests read the fixture pages `../tests/fixtures/*.html` (`?raw`), outside this package.
// html-validate's browser build needs no DOM: the source is parsed by html-validate itself.
export default defineConfig({
  server: { fs: { allow: ['..'] } },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
