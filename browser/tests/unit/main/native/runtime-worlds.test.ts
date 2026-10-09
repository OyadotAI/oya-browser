/** Isolated-world allocation has bounded ownership and cannot resurrect disposed contexts. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { RuntimeWorlds } from '../../../../src/main/native/runtime-worlds.ts';
import { RUNTIME } from '../../../../src/main/native/runtime-types.ts';
/** Minimal exact-document metadata, without a browser or debugging transport. */
const main = (id = 'document') =>
  ({ id: 1, uniqueId: id, document: id, frame: { detached: false }, frameId: 'frame', target: 'tab' }) as any;

test('concurrent named creation coalesces while unnamed worlds stay independent', async () => {
  const worlds = new RuntimeWorlds(async () => {});
  const parent = main();
  let resolve!: (reply: any) => void;
  let calls = 0;
  const create = () => {
    calls++;
    return new Promise<any>((r) => {
      resolve = r;
    });
  };
  const a = worlds.create(parent, 'name', create);
  const b = worlds.create(parent, 'name', create);
  assert.equal(calls, 1);
  resolve({ context: 'isolated' });
  assert.equal(await a, await b);
  const c = await worlds.create(parent, '', async () => ({ context: 'unnamed-one' }));
  const d = await worlds.create(parent, '', async () => ({ context: 'unnamed-two' }));
  assert.notEqual(c.id, d.id);
  assert.notEqual(c.world, d.world);
  assert.equal(worlds.live([parent]).length, 3);
  assert.deepEqual(worlds.live([main('replacement')]), []);
});

test('closed owners revoke late native worlds before rejecting their allocation', async () => {
  const released: any[] = [];
  const worlds = new RuntimeWorlds(async (...args) => {
    released.push(args);
  });
  let resolve!: (reply: any) => void;
  const pending = worlds.create(
    main(),
    'name',
    () =>
      new Promise((r) => {
        resolve = r;
      }),
  );
  worlds.dispose();
  resolve({ context: 'late-token' });
  await assert.rejects(pending, /closed/);
  assert.equal(released.length, 1);
  assert.equal(released[0][2], 'late-token');
  assert.deepEqual(worlds.all(), []);
  await assert.rejects(
    worlds.create(main(), 'next', async () => ({ context: 'next' })),
    /closed/,
  );
});

test('creation refuses older engines rather than treating the main world as isolated', async () => {
  const worlds = new RuntimeWorlds(async () => {});
  for (const reply of [{}, { context: 'document' }])
    await assert.rejects(
      worlds.create(main(), 'name', async () => reply),
      /lacks isolated/,
    );
  assert.deepEqual(worlds.all(), []);
  assert.ok(await worlds.create(main(), 'name', async () => ({ context: 'real-isolated' })));
});

test('quota reserves in-flight allocations and replacement documents release old public entries', async () => {
  const worlds = new RuntimeWorlds(async () => {});
  const parent = main();
  const pending = Array.from({ length: RUNTIME.worlds }, (_, i) =>
    worlds.create(parent, String(i), async () => ({ context: String(i) })),
  );
  await assert.rejects(
    worlds.create(parent, 'over-limit', async () => ({ context: 'overflow' })),
    /limit/,
  );
  await Promise.all(pending);
  const replacement = { ...parent, uniqueId: 'new', document: 'new' };
  worlds.prune(replacement);
  assert.deepEqual(worlds.all(), []);
  assert.ok(await worlds.create(replacement, 'after-navigation', async () => ({ context: 'new-world' })));
});
