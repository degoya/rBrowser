import vue from '@vitejs/plugin-vue';
import ui from '@nuxt/ui/vite';
import { defineConfig } from 'vitest/config';

// The panel runtime's tests, with the components compiled as a panel's build compiles them.
export default defineConfig({
  plugins: [vue(), ui({ router: false, colorMode: false, dts: false, ui: { colors: { primary: 'teal', neutral: 'slate' } } })],
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts'],
    setupFiles: ['src/test-setup.ts'],
  },
});
