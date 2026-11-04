/** Windows agent discovery and direct execution keep paths and credentials out of command shells. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import {
  agentCommand,
  agentBinDirs,
  vscodeConfig,
  runAgent,
  type AgentPlatform,
} from '../../../../src/main/local-mcp/agent-platform.ts';

/** Synthetic Windows installation paths never access this computer's agent settings. */
function windows(files: string[] = []): AgentPlatform {
  return {
    platform: 'win32',
    home: 'C:\\Users\\Test User',
    env: { APPDATA: 'C:\\Users\\Test User\\AppData\\Roaming', Path: 'D:\\Tools;C:\\Program Files\\nodejs' },
    exists: (file) => files.includes(file),
  };
}
it('finds a native Claude installation with Windows path separators and a mixed-case Path variable', () => {
  const deps = windows(['D:\\Tools\\claude.exe']);
  assert.deepEqual(agentCommand('claude', deps), { file: 'D:\\Tools\\claude.exe', args: [] });
  assert(agentBinDirs(deps).includes('C:\\Users\\Test User\\.local\\bin'));
  assert.equal(vscodeConfig(deps), 'C:\\Users\\Test User\\AppData\\Roaming\\Code\\User\\mcp.json');
});
it('runs a known npm entrypoint with Node and refuses an arbitrary command shim', () => {
  const root = 'C:\\Users\\Test User\\AppData\\Roaming\\npm';
  const entry = `${root}\\node_modules\\@anthropic-ai\\claude-code\\cli.js`;
  const deps = windows([`${root}\\claude.cmd`, entry, 'C:\\Program Files\\nodejs\\node.exe']);
  assert.deepEqual(agentCommand('claude', deps), { file: 'C:\\Program Files\\nodejs\\node.exe', args: [entry] });
  assert.equal(agentCommand('unknown', deps), null);
  assert.equal(agentCommand('claude', windows([`${root}\\claude.cmd`])), null);
});
it('preserves shell punctuation as literal arguments and bounds execution', async () => {
  const deps = windows(['D:\\Tools\\claude.exe']);
  const calls: any[] = [];
  deps.run = async (...args) => {
    calls.push(args);
  };
  const args = ['mcp', 'add', '--header', 'Authorization: Bearer synthetic-&|%token', 'http://127.0.0.1:47321/mcp'];
  await runAgent('claude', args, deps);
  assert.deepEqual(calls[0][1], args);
  assert.equal(calls[0][2].shell, undefined);
  assert.equal(calls[0][2].timeout, 30000);
  assert(!Object.hasOwn(calls[0][2].env, 'Path'));
});
it('reports missing and failing runtimes without leaking process arguments', async () => {
  await assert.rejects(runAgent('claude', [], windows()), /not installed/);
  const deps = windows(['D:\\Tools\\claude.exe']);
  deps.run = async () => {
    throw Error('synthetic-secret-in-stderr');
  };
  await assert.rejects(
    runAgent('claude', ['synthetic-secret-in-stderr'], deps),
    (error: Error) => !error.message.includes('synthetic-secret'),
  );
});
