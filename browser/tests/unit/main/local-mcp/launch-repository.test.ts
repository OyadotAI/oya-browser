/** Terminal handoffs keep executable arguments as private data, including hostile punctuation, without shell evaluation. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { LaunchRepository } from '../../../../src/main/local-mcp/launch-repository.ts';

it('the static macOS wrapper preserves argument boundaries without evaluating shell syntax', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-agent-wrapper-test-'));
  const script = path.join(root, "agent 'quoted' $.mjs"),
    output = path.join(root, 'arguments.json');
  fs.writeFileSync(
    script,
    'import fs from "node:fs"; fs.writeFileSync(process.env.OYA_AGENT_TEST_OUTPUT, JSON.stringify(process.argv.slice(2)));',
  );
  const args = [script, 'spaces in one argument', '$(touch should-never-exist)', '`whoami`', 'a; exit 99'];
  const lease = new LaunchRepository().prepare({ file: process.execPath, args }, 'darwin');
  try {
    assert.equal(fs.statSync(lease.file).mode & 0o777, 0o700);
    assert(!fs.readFileSync(lease.file, 'utf8').includes(script));
    await promisify(execFile)('/bin/sh', [lease.file], {
      cwd: root,
      env: { ...process.env, OYA_AGENT_TEST_OUTPUT: output },
    });
    assert.deepEqual(JSON.parse(fs.readFileSync(output, 'utf8')), args.slice(1));
    assert(!fs.existsSync(path.join(root, 'should-never-exist')));
    assert(lease.started());
  } finally {
    lease.close();
    fs.rmSync(root, { recursive: true, force: true });
  }
});
it('Windows handoff uses a JSON argument manifest without changing execution policy or submitting a task', () => {
  const command = { file: 'C:\\Users\\A&B\\node.exe', args: ['C:\\Some path\\cli.js'] };
  const lease = new LaunchRepository().prepare(command, 'win32');
  const dir = path.dirname(lease.file);
  try {
    const text = fs.readFileSync(lease.file, 'utf8');
    assert(!text.includes(command.file));
    assert(!text.includes('ExecutionPolicy'));
    assert(!text.includes('GitHub'));
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(dir, 'manifest.json'), 'utf8')), command);
  } finally {
    lease.close();
  }
  assert(!fs.existsSync(dir));
});
it('unsupported newline arguments are refused before creating a handoff', () => {
  assert.throws(
    () => new LaunchRepository().prepare({ file: '/installed/agent', args: ['line\nbreak'] }, 'darwin'),
    /manually/,
  );
});
