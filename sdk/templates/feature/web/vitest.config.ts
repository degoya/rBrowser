import { defineConfig, mergeConfig } from 'vitest/config';
import panel from './vite.config.ts';

// The panel's components compiled as its build compiles them, in a stand-in DOM.
export default mergeConfig(
  panel,
  defineConfig({
    test: {
      environment: 'happy-dom',
      include: ['src/**/*.test.ts'],
      setupFiles: ['./web-common/panel-ui/src/test-setup.ts'],
    },
  }),
);
