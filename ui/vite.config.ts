import { fileURLToPath, URL } from 'node:url';
import vue from '@vitejs/plugin-vue';
import ui from '@nuxt/ui/vite';
import { defineConfig } from 'vitest/config';
import { BUNDLED_ICONS } from './src/icons.ts';
import { SETTINGS_BUNDLED_ICONS } from './src/settings/icons.ts';

// The shell is loaded from the custom scheme `rbrowser://ui/...` (RB-010-04), so every asset
// path is relative (`base: './'`). Four pages: the toolbar (`index.html`), the pages window
// (`settings.html#/<route>`, RB-010-08: settings, trust, the analyser's wizard), the status bar
// (`status-bar.html`, RB-080-15) and the canvas board behind the canvas's views
// (`canvas-board.html`, RB-080-04). The label strips of the cells are native (RB-090-27); every
// page here is loaded by a view (`src/pages.test.ts`).
export default defineConfig({
  base: './',
  plugins: [
    vue(),
    ui({
      router: false,
      // The theme follows the `ui.theme` setting (src/theme), not a localStorage key.
      colorMode: false,
      ui: { colors: { primary: 'teal', neutral: 'slate' } },
      icon: { clientBundle: { icons: [...BUNDLED_ICONS, ...SETTINGS_BUNDLED_ICONS], scan: true } },
    }),
  ],
  resolve: {
    alias: {
      '@locales': fileURLToPath(new URL('../locales', import.meta.url)),
      '@assets': fileURLToPath(new URL('../assets', import.meta.url)),
    },
  },
  server: { fs: { allow: ['..'] } },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL('./index.html', import.meta.url)),
        'canvas-board': fileURLToPath(new URL('./canvas-board.html', import.meta.url)),
        settings: fileURLToPath(new URL('./settings.html', import.meta.url)),
        'status-bar': fileURLToPath(new URL('./status-bar.html', import.meta.url)),
      },
    },
  },
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.ts'],
    // A give-up deadline, not a measure: the tests that mount the whole settings page take
    // 2 s alone and ran past the 5 s default while the Rust lanes built beside them (wave c20).
    testTimeout: 30_000,
  },
});
