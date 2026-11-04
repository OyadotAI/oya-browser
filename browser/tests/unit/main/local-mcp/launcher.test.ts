/** Native launch adapters accept known installations only and release bounded handoffs on every exit. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { AgentLauncher } from '../../../../src/main/local-mcp/launcher.ts';

/** Native discovery sees only the synthetic editor and CLI paths. */
function platform() {
  return {
    platform: 'darwin',
    home: '/synthetic-home',
    env: { PATH: '/installed' },
    exists: (file) => file === '/installed/claude' || file === '/Applications/Cursor.app',
  };
}
it('opens an editor directly and a CLI through a private document without prompt arguments', async () => {
  const opened: string[] = [],
    commands: any[] = [];
  let closed = 0;
  const files: any = {
    prepare: (command) => {
      commands.push(command);
      return { file: '/private/owned/open-agent.command', started: () => true, close: () => closed++ };
    },
  };
  const launcher = new AgentLauncher(
    {
      openPath: async (file) => {
        opened.push(file);
        return '';
      },
    },
    platform(),
    files,
  );
  await launcher.open('cursor');
  await launcher.open('claude-code');
  assert.deepEqual(opened, ['/Applications/Cursor.app', '/private/owned/open-agent.command']);
  assert.deepEqual(commands, [{ file: '/installed/claude', args: [] }]);
  assert.equal(closed, 1);
});
it('unknown IDs and unavailable installations never reach the native opener', async () => {
  let opened = 0;
  const launcher = new AgentLauncher(
    {
      openPath: async () => {
        opened++;
        return '';
      },
    },
    platform(),
  );
  await assert.rejects(launcher.open('/bin/sh'), /manually/);
  await assert.rejects(launcher.open('codex'), /manually/);
  assert.equal(opened, 0);
});
it('native refusal cleans the private handoff and keeps a manual recovery path', async () => {
  let closed = 0;
  const files: any = {
    prepare: () => ({ file: '/private/owned/agent.command', started: () => false, close: () => closed++ }),
  };
  const launcher = new AgentLauncher({ openPath: async () => 'OS refused' }, platform(), files);
  await assert.rejects(launcher.open('claude-code'), /manually/);
  assert.equal(closed, 1);
});
it('shutdown settles a stalled native opener without stopping already opened apps', async () => {
  let closed = 0;
  const files: any = {
    prepare: () => ({ file: '/private/owned/agent.command', started: () => false, close: () => closed++ }),
  };
  const launcher = new AgentLauncher({ openPath: () => new Promise(() => {}) }, platform(), files);
  const pending = launcher.open('claude-code'),
    failed = assert.rejects(pending, /manually/);
  launcher.dispose();
  await failed;
  assert.equal(closed, 1);
});
