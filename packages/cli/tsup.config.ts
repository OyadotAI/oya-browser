/**
 * Build for the `oya` bin: one ESM file with a shebang, the SDK bundled in.
 */
import { defineConfig } from 'tsup';

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['esm'],
  clean: true,
  // The bin must be directly executable, and @oya-ai/browser is bundled in so
  // `npx @oya-ai/cli` is one download rather than two.
  banner: { js: '#!/usr/bin/env node' },
  noExternal: ['@oya-ai/browser'],
});
