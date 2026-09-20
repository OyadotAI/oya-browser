/**
 * Unit tests for the console: components and hooks rendered in jsdom with
 * Testing Library. End-to-end flows stay in Playwright (tests/*.spec.ts).
 */
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  test: {
    environment: 'jsdom',
    include: ['tests/unit/**/*.test.{ts,tsx}'],
    restoreMocks: true,
    // Node 25 defines its own (unconfigured) localStorage global, which shadows
    // jsdom's; switching Node's off lets the page code see a working one.
    execArgv: ['--no-experimental-webstorage'],
    coverage: { include: ['src/**'], reporter: ['text-summary'] },
  },
});
