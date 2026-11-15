import { defineConfig, mergeConfig } from 'vitest/config';
import panel from './vite.config.ts';

// The panel's components compiled as its build compiles them; the tests read the page script
// (../src/page.js) and the fixture form from outside this folder, and the document is the
// fixture page's address.
export default mergeConfig(
  panel,
  defineConfig({
    server: { fs: { allow: ['..', '../../..'] } },
    test: {
      environment: 'happy-dom',
      environmentOptions: { happyDOM: { url: 'http://127.0.0.1:18767/forms/register.html' } },
      include: ['src/**/*.test.ts'],
      setupFiles: ['../../web-common/panel-ui/src/test-setup.ts'],
    },
  }),
);
