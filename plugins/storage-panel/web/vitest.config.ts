import { defineConfig, mergeConfig } from 'vitest/config';
import panel from './vite.config.ts';

// The panel's components compiled as its build compiles them; the tests read the plugin's
// files (the page script, the component's storage operations) from outside this folder.
export default mergeConfig(
  panel,
  defineConfig({
    server: { fs: { allow: ['..', '../../..'] } },
    test: {
      environment: 'happy-dom',
      environmentOptions: { happyDOM: { url: 'http://127.0.0.1:18767/storage/index.html' } },
      include: ['src/**/*.test.ts'],
      setupFiles: ['../../web-common/panel-ui/src/test-setup.ts'],
    },
  }),
);
