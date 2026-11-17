// Exported by rBrowser (recorder-export) from the recording "login": one project per
// breakpoint. RB_BASE_URL runs the test against another origin.
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  fullyParallel: true,
  use: {
    baseURL: process.env.RB_BASE_URL ?? 'http://127.0.0.1:8000',
  },
  projects: [
    {
      name: 'mobile',
      use: {
        browserName: 'chromium',
        viewport: { width: 390, height: 844 },
        deviceScaleFactor: 3,
        isMobile: true,
        hasTouch: true,
        locale: 'en-US',
      },
    },
    {
      name: 'tablet',
      use: {
        browserName: 'chromium',
        viewport: { width: 820, height: 1180 },
        deviceScaleFactor: 2,
        isMobile: true,
        hasTouch: true,
        colorScheme: 'light',
      },
    },
    {
      name: 'desktop',
      use: {
        browserName: 'chromium',
        viewport: { width: 1440, height: 900 },
        deviceScaleFactor: 1,
        isMobile: false,
        hasTouch: false,
        locale: 'de-DE',
        colorScheme: 'dark',
        reducedMotion: 'reduce',
      },
    },
  ],
});
