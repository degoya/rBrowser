import { defineConfig } from 'vitest/config';

// The loader's tests read a sibling plugin's files (plugins/console) as that plugin's tests would;
// the JSON parity test imports the shell's tokenizer (ui/src/settings/jsonTokens.ts).
export default defineConfig({
  server: { fs: { allow: ['..', '../..'] } },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts'],
  },
});
