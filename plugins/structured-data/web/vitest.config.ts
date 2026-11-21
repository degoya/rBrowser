import { defineConfig, mergeConfig } from 'vitest/config';
import panel from './vite.config.ts';

// The panel's components compiled as its build compiles them; the tests read the plugin's
// files from outside this folder.
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
