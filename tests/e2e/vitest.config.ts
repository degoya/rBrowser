// The harness's own helpers only: the K cases need the Windows app (scripts/e2e-windows.sh), and
// recorder/ is K12's Playwright project with tests of its own.
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['lib/**/*.test.ts', '*.test.ts'],
    exclude: ['recorder/**', 'node_modules/**'],
  },
});
