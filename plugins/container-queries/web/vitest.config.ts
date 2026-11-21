import { defineConfig, mergeConfig } from 'vitest/config';
import panel from './vite.config.ts';

// The unit tests: the logic of the page script and the panel in happy-dom (no layout; the
// fixture's sizes are given), the panel's components compiled as its build compiles them. The
// real layout is the e2e suite (vitest.e2e.config.ts).
export default mergeConfig(
  panel,
  defineConfig({
    server: { fs: { allow: ['..', '../../..'] } },
    test: {
      environment: 'happy-dom',
      include: ['src/**/*.test.ts'],
      setupFiles: ['../../web-common/panel-ui/src/test-setup.ts'],
    },
  }),
);
