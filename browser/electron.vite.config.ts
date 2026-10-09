/**
 * The build: the main process, the workflow validation worker (forked by the
 * main process as a utility process), the shell's preload, and the shell's two
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

/** The worker's own source (Vite's module ids use forward slashes on every OS). */
const WORKER_SOURCE = '/src/worker/';

/** What Rollup tells a plugin about one dynamic import: the importing module, and the target when it is static. */
interface DynamicImport {
  /** The importing module's id (a file path). */
  moduleId: string;
  /** The imported module's id, or null for a computed specifier. */
  targetModuleId: string | null;
}

/** The worker's computed import() rendered as it is, so Node loads the generated ES module. */
const renderWorkerImport = ({ moduleId, targetModuleId }: DynamicImport) =>
  moduleId.includes(WORKER_SOURCE) && !targetModuleId ? { left: 'import(', right: ')' } : null;

/**
 * The worker loads each generated workflow module by file URL (an ES module), which
 * require() cannot load, so its computed dynamic imports keep their import().
 */
const keepWorkerImports = { name: 'keep-worker-imports', renderDynamicImport: renderWorkerImport };

/** Bundled entries, each written to `<outDir>/<name>.js`. */
const entry = (input: Record<string, string>, outDir: string) => ({
  plugins: [keepWorkerImports],
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
  // out/main/worker.js is the validation worker the main process forks (src/main/app/boot.ts).
  main: entry({ index: 'src/main/main.ts', worker: 'src/worker/index.ts' }, 'out/main'),
  preload: entry(
    { index: 'src/preload/index.ts', recording: 'src/preload/recording.ts', dialog: 'src/preload/dialog.ts' },
    'out/preload',
  ),
  renderer,
});
