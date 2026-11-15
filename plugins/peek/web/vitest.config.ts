import { defineConfig } from 'vitest/config';

// The page script imports the sync agent's selector code (agent/src/) and the plugin's locales,
// both outside this package; the freshness test reads the built files beside it.
export default defineConfig({
  server: { fs: { allow: ['../../..'] } },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts'],
  },
});
