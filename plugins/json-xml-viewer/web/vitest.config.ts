import { defineConfig } from 'vitest/config';

// The page script bundles the plugin's locales (../locales, outside this package); the freshness
// test reads the built page script beside it.
export default defineConfig({
  server: { fs: { allow: ['..'] } },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts'],
  },
});
