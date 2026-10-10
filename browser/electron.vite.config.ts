/**
 * The build: the native main process, the shell's preload, and the shell's two
 * pages (the shell and the control shield, React and TypeScript), bundled into out/.
 * Runtime dependencies (package.json "dependencies") stay external and ship in
 * node_modules; page-side and server-shared files ship as they are (see
 * package.json build.files).
 */
import { resolve } from 'node:path';
import { defineConfig } from 'electron-vite';
import react from '@vitejs/plugin-react';
import { createRequire } from 'node:module';

/** Load the packaging hook as CommonJS rather than bundling its Node filesystem imports. */
const { configuredAccessGroup } = createRequire(import.meta.url)('./build/webauthn.cjs');

/** The JavaScript the main process loads (anonymity/) is CommonJS, so the CommonJS transform covers it. */
const COMMONJS = { include: [/\.c?js$/], strictRequires: true, ignoreDynamicRequires: true };

/**
 * A dynamic import of a runtime dependency (electron-updater, loaded only once a
 * packaged app starts updating) stays a lazy require(): Node's own import() of a
 * CommonJS package misses exports it defines through getters, autoUpdater among them.
 */
const OUTPUT = { dynamicImportInCjs: false };

/** Bundled entries, each written to `<outDir>/<name>.js`. */
const entry = (input: Record<string, string>, outDir: string) => ({
  define: { __OYA_WEBAUTHN_GROUP__: JSON.stringify(configuredAccessGroup()) },
  build: { outDir, commonjsOptions: COMMONJS, rollupOptions: { input, output: OUTPUT } },
});

/** The shell's pages, loaded from disk: relative URLs, and public/ copied beside them as is. */
const renderer = {
  root: 'src/renderer',
  base: './',
  publicDir: resolve('src/renderer/public'),
  plugins: [react()],
  build: {
    outDir: resolve('out/renderer'),
    rollupOptions: {
      input: {
        index: resolve('src/renderer/index.html'),
        shield: resolve('src/renderer/control-shield/index.html'),
        preview: resolve('src/renderer/tab-preview/index.html'),
      },
    },
  },
};

export default defineConfig({
  main: entry({ index: 'src/main/main.ts' }, 'out/main'),
  preload: entry(
    { index: 'src/preload/index.ts', recording: 'src/preload/recording.ts', dialog: 'src/preload/dialog.ts' },
    'out/preload',
  ),
  renderer,
});
