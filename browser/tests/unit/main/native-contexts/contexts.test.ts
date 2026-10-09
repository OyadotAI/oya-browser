/** Ephemeral native sessions cannot leak between owners, survive disconnect or expose failed setup. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeContexts, privateSession } from '../../../../src/main/native-contexts/index.ts';
/** Native session seam records real cleanup stages without opening a browser. */
function fixture(configure: () => Promise<void> = async () => {}) {
  const calls: string[] = [],
    partitions: string[] = [];
  const session = {
    closeAllConnections: async () => {
      calls.push('connections');
    },
    clearStorageData: async () => {
      calls.push('storage');
    },
    clearCache: async () => {
      calls.push('cache');
    },
  } as any;
  const contexts = new NativeContexts({
    create: (partition) => {
      partitions.push(partition);
      return session;
    },
    configure,
    close: () => {
      calls.push('tabs');
    },
  });
  return { contexts, session, calls, partitions };
}
test('context ownership is private and tombstoned sessions never become public', async () => {
  const a = fixture(),
    b = fixture(),
    id = await a.contexts.create();
  assert.equal(a.partitions[0].startsWith('persist:'), false);
  assert.deepEqual(a.contexts.list(), [id]);
  assert.equal(a.contexts.get(id), a.session);
  assert.throws(() => b.contexts.get(id), /foreign/);
  assert.equal(b.contexts.visible(a.session), false);
  assert.equal(privateSession(a.session), true);
  await a.contexts.remove(id);
  assert.equal(a.contexts.visible(a.session), false);
  assert.equal(b.contexts.visible(a.session), false);
  assert.deepEqual(a.calls, ['tabs', 'connections', 'storage', 'cache']);
});
test('failed setup closes tabs and clears the native session before rejecting', async () => {
  const f = fixture(async () => {
    throw Error('policy failed');
  });
  await assert.rejects(f.contexts.create(), /policy failed/);
  assert.deepEqual(f.contexts.list(), []);
  assert.deepEqual(f.calls, ['tabs', 'connections', 'storage', 'cache']);
});
test('disconnect fences late setup and cleans newly installed resources a second time', async () => {
  let resume!: () => void;
  const f = fixture(
    () =>
      new Promise<void>((resolve) => {
        resume = resolve;
      }),
  );
  const created = f.contexts.create();
  const rejected = assert.rejects(created, /disconnected/);
  await f.contexts.dispose();
  resume();
  await rejected;
  assert.deepEqual(f.contexts.list(), []);
  assert.deepEqual(f.calls, ['tabs', 'connections', 'storage', 'cache', 'tabs', 'connections', 'storage', 'cache']);
  await assert.rejects(f.contexts.create(), /closed/);
});
