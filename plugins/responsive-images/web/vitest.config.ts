import { defineConfig, mergeConfig } from 'vitest/config';
import panel from './vite.config.ts';

// The panel's components compiled as its build compiles them. The collector imports the sync
// agent's selector code (agent/src/), outside this package; the freshness and fixture tests read
// the built collector and the Rust fixtures beside it.
export default mergeConfig(
  panel,
  defineConfig({
    server: { fs: { allow: ['../../..'] } },
    test: {
      environment: 'happy-dom',
      include: ['src/**/*.test.ts'],
      setupFiles: ['../../web-common/panel-ui/src/test-setup.ts'],
    },
  }),
);
