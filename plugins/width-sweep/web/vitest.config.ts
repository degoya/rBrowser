import { defineConfig } from 'vitest/config';

// The detector's logic in happy-dom, which has no layout: each test gives the geometry. The
// freshness test reads the built ../src/detect.js beside the package.
export default defineConfig({
  server: { fs: { allow: ['..'] } },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts'],
  },
});
