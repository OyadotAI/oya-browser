/** Library IPC stays on the authenticated project's allowlisted API routes. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { PlaybookHandlers } from '../../../../src/main/ipc/playbooks.ts';
import { ServerApi } from '../../../../src/main/connection/server-api.ts';
/** Minimal service seam: tests stub all server I/O. */
const deps = () =>
  ({
    config: { values: { apiKey: 'test' } },
    socket: { ready: true, browserId: 'b1' },
    recorder: {},
    control: { snapshot: () => ({ mode: 'agent' }) },
  }) as any;
it('rejects unknown commands and disconnected library access', async () => {
  const ctx = deps();
  const handlers = new PlaybookHandlers(ctx);
  assert.match(String((await handlers.call({ action: 'constructor' })).error), /Unknown/);
  ctx.socket.ready = false;
  assert.match(String((await handlers.call({ action: 'list' })).error), /Reconnect/);
});
it('escapes names and forwards rename to the existing project endpoint', async (t) => {
  const calls: unknown[][] = [];
  t.mock.method(ServerApi.prototype, 'send', async (...args: unknown[]) => {
    calls.push(args);
    return { ok: true };
  });
  await new PlaybookHandlers(deps()).call({ action: 'rename', name: 'lookup/2026', to: 'lookup-new' });
  assert.deepEqual(calls, [['PATCH', 'playbooks/lookup%2F2026', { name: 'lookup-new' }]]);
});
it('reports server failures without claiming a library edit succeeded', async (t) => {
  t.mock.method(ServerApi.prototype, 'send', async () => {
    throw new Error('Read-only project');
  });
  assert.deepEqual(await new PlaybookHandlers(deps()).call({ action: 'delete', name: 'lookup' }), {
    error: 'Read-only project',
  });
});
