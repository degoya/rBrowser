import { defineConfig, mergeConfig } from 'vitest/config';
import panel from './vite.config.ts';

// The page script's logic and the picker in happy-dom (no layout, no filters drawn), the picker's
// components compiled as its build compiles them; the real browser is test:e2e's
// (vitest.e2e.config.ts). `isObject` and panel-ui come from plugins/web-common.
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
