import { defineConfig } from 'vitest/config';

// The freshness test reads the committed bundles of ../page and ../panel, outside this folder;
// the validator of the probe's report is the agent's (../../../agent/src/framework/report.ts).
export default defineConfig({
  server: { fs: { allow: ['..', '../..', '../../..'] } },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    testTimeout: 30_000,
  },
});
