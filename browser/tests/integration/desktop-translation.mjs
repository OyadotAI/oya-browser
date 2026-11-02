/** Build the production UI fixture and run it only in branded Oya with a disposable profile. */
import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
import { developmentExecutable } from '../../src/dev/launch.ts';

const profile = await mkdtemp(join(tmpdir(), 'oya-translation-'));
await build({
  entryPoints: [fileURLToPath(new URL('./desktop-translation-renderer.tsx', import.meta.url))],
  bundle: true,
  outfile: join(profile, 'fixture.js'),
  jsx: 'automatic',
  loader: { '.woff2': 'file' },
  define: { 'process.env.NODE_ENV': '"production"' },
});
await writeFile(
  join(profile, 'index.html'),
  '<!doctype html><meta charset="utf-8"><link rel="stylesheet" href="fixture.css"><style>#backdrop{position:absolute;inset:88px 0 0;background-size:100% 100%}.toolbar{position:relative;z-index:1}</style><body class="mode-browsing"><div id="backdrop"></div><div id="root"></div><script src="fixture.js"></script>',
);
const fixture = fileURLToPath(new URL('./desktop-translation-electron.cjs', import.meta.url));
const child = spawn(developmentExecutable(), [fixture], {
  stdio: 'inherit',
  env: { ...process.env, OYA_TRANSLATION_PROFILE: profile },
});
child.on('error', (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on('close', async (code) => {
  await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  process.exitCode = code ?? 1;
});
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
