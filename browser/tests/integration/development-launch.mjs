/** Verify the real macOS Launch Services command with a disposable, device-free Oya application. */
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { developmentCommand, developmentExecutable } from '../../src/dev/launch.ts';

if (process.platform !== 'darwin') throw Error('This native development-launch fixture requires macOS');
const scratch = fs.mkdtempSync(path.join(tmpdir(), 'oya-launch-smoke-'));
const fixture = path.join(scratch, 'fixture.cjs');
const result = path.join(scratch, 'result.json');
const logs = ['stdout.log', 'stderr.log'].map((name) => path.join(scratch, name));
for (const log of logs) fs.writeFileSync(log, '', { mode: 0o600 });
fs.writeFileSync(
  fixture,
  `/** This owned fixture verifies launching without websites or devices. */
const { app } = require('electron');
const fs = require('node:fs');
app.setPath('userData', process.env.OYA_LAUNCH_SMOKE_PROFILE);
fs.writeFileSync(process.env.OYA_LAUNCH_SMOKE_RESULT, JSON.stringify({marker:process.env.OYA_LAUNCH_SMOKE_MARKER,args:process.argv.slice(2)}));
process.stdout.write('oya-launch-stdout-ok\\n');
process.stderr.write('oya-launch-stderr-ok\\n');
app.whenReady().then(() => app.exit(0));
`,
);
const env = Object.fromEntries(
  Object.entries(process.env).filter(([key]) => !key.startsWith('OYA_') && key !== 'ELECTRON_RUN_AS_NODE'),
);
Object.assign(env, {
  OYA_LAUNCH_SMOKE_PROFILE: path.join(scratch, 'profile'),
  OYA_LAUNCH_SMOKE_RESULT: result,
  OYA_LAUNCH_SMOKE_MARKER: 'synthetic inherited marker',
});
const args = ['--dev', 'argument with spaces'];
const request = developmentCommand(developmentExecutable(), [fixture, ...args], logs);
const run = spawnSync(request.command, request.args, { env, shell: false, encoding: 'utf8', timeout: 30000 });
assert.equal(run.error, undefined, `Launch failed; owned evidence retained at ${scratch}`);
assert.equal(run.status, 0, run.stderr);
try {
  assert.deepEqual(JSON.parse(fs.readFileSync(result, 'utf8')), { marker: 'synthetic inherited marker', args });
  assert.match(fs.readFileSync(logs[0], 'utf8'), /oya-launch-stdout-ok/);
  assert.match(fs.readFileSync(logs[1], 'utf8'), /oya-launch-stderr-ok/);
  console.log(
    'Oya Launch Services: inherited environment, exact arguments, private stdout/stderr and normal waiter completion passed. No devices or websites used.',
  );
} finally {
  fs.rmSync(scratch, { recursive: true, force: true });
}
