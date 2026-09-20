import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  // tests/unit holds the Vitest suites; Playwright runs only the end-to-end specs.
  testIgnore: 'unit/**',
  use: {
    baseURL: process.env.OYA_TEST_URL || 'http://localhost:3100',
    channel: 'chrome',
    viewport: { width: 1440, height: 900 },
  },
});
