/** Run the release acceptance matrix only through an explicitly supplied native Oya engine. */
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import path from 'node:path';
import os from 'node:os';

/** Every fixture requires OYA_NATIVE_ENGINE and forbids alternative browser providers. */
const SUITES = [
  'native-pre-scripts',
  'native-app',
  'native-policy',
  'native-platform',
  'native-worker-client-hints',
  'native-contexts',
  'native-storage',
  'native-runtime-worlds',
  'native-front-door',
  'native-input',
  'native-frames',
  'native-drag',
  'native-choosers',
  'native-dialogs',
  'native-recording-transport',
  'native-workflow',
  'analyzer-dom',
  'recorder-dom',
];
/** Bound each suite so a hung renderer cannot silently strand release preparation. */
const SUITE_TIMEOUT_MS = 180_000;
const browser = path.resolve(import.meta.dirname, '..');
const engine = process.env.OYA_NATIVE_ENGINE;
if (!engine || !path.isAbsolute(engine))
  throw Error('Set OYA_NATIVE_ENGINE to the absolute patched Oya executable; no fallback is permitted');
const requested = process.argv.slice(2);
const suites = requested.length ? requested : SUITES;
for (const suite of suites) if (!SUITES.includes(suite)) throw Error('Unknown native acceptance suite: ' + suite);
const output = path.resolve(
  process.env.OYA_ACCEPTANCE_OUTPUT || path.join(os.tmpdir(), 'oya-native-acceptance-' + Date.now()),
);
await mkdir(output, { recursive: true });
const report = {
  engine,
  platform: process.platform,
  architecture: process.arch,
  startedAt: new Date().toISOString(),
  finishedAt: null,
  status: 'running',
  suites: [],
};
/** Save every completed result so interruption never leaves a misleading all-pass report. */
async function save() {
  await writeFile(path.join(output, 'report.json'), JSON.stringify(report, null, 2) + '\n');
}
/** Preserve each fixture's complete diagnostics without printing credentials or overwhelming the terminal. */
function execute(suite) {
  return new Promise((resolve) => {
    const started = Date.now();
    const logfile = path.join(output, suite + '.log');
    const log = createWriteStream(logfile);
    const child = spawn(process.execPath, ['tests/integration/' + suite + '.mjs'], {
      cwd: browser,
      env: { ...process.env, ELECTRON_RUN_AS_NODE: '' },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let timedOut = false,
      failure;
    const deadline = setTimeout(() => {
      timedOut = true;
      child.kill('SIGTERM');
    }, SUITE_TIMEOUT_MS);
    child.stdout.pipe(log, { end: false });
    child.stderr.pipe(log, { end: false });
    child.once('error', (error) => {
      failure = error.message;
    });
    child.once('close', (code, signal) => {
      clearTimeout(deadline);
      log.end();
      resolve({
        name: suite,
        status: code === 0 && !timedOut && !failure ? 'passed' : 'failed',
        code,
        signal,
        timedOut,
        failure,
        elapsedMs: Date.now() - started,
        log: logfile,
      });
    });
  });
}
await save();
for (const suite of suites) {
  console.log('Native Oya acceptance: ' + suite);
  const result = await execute(suite);
  report.suites.push(result);
  await save();
  console.log(result.status + ': ' + suite + ' (' + result.elapsedMs + 'ms)');
}
report.finishedAt = new Date().toISOString();
report.status = report.suites.every((suite) => suite.status === 'passed') ? 'passed' : 'failed';
await save();
console.log('Native acceptance report: ' + path.join(output, 'report.json'));
process.exitCode = report.status === 'passed' ? 0 : 1;
