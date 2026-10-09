/** Native context selection is exact-document and exact-tab even across delayed engine replies. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { RuntimeContexts } from '../../../../src/main/native/runtime-contexts.ts';
/** Mutable native graph seam models removal without URL-based lookup. */
function fixture() {
  const make = (document: string) => ({
    detached: false,
    document,
    async _runOyaRuntime() {
      return { context: this.document };
    },
  });
  const top = Object.assign(make('top'), { framesInSubtree: [] as any[] });
  const child = make('child');
  top.framesInSubtree = [top, child];
  const page = { webContents: { mainFrame: top, isDestroyed: () => false } } as any;
  return { top, child, page, contexts: new RuntimeContexts(() => 'child-frame') };
}
test('child context identity matches the shared frame graph and cannot select another target', async () => {
  const f = fixture();
  const [main, child] = await f.contexts.all(f.page, 'tab');
  assert.equal(main.frameId, 'tab');
  assert.equal(child.frameId, 'child-frame');
  assert.equal(f.contexts.selected('tab', { contextId: child.id }), child);
  assert.throws(() => f.contexts.selected('other', { contextId: child.id }), /foreign/);
  f.contexts.dispose();
});
test('replacing a native child document revokes its public context while preserving its sibling', async () => {
  const f = fixture();
  const [main, old] = await f.contexts.all(f.page, 'tab');
  f.child.document = 'replacement';
  const next = await f.contexts.current(f.page, 'tab', f.child as any);
  assert.notEqual(next.uniqueId, old.uniqueId);
  assert.throws(() => f.contexts.selected('tab', { contextId: old.id }), /foreign/);
  assert.equal(f.contexts.selected('tab', { contextId: main.id }), main);
  f.contexts.dispose();
});
test('removal during native discovery cannot publish a context for a no-longer-owned frame', async () => {
  const f = fixture();
  let resolve!: (value: any) => void;
  f.child._runOyaRuntime = () =>
    new Promise((r) => {
      resolve = r;
    });
  const pending = f.contexts.current(f.page, 'tab', f.child as any);
  f.top.framesInSubtree = [f.top];
  resolve({ context: 'removed' });
  await assert.rejects(pending, /foreign/);
  assert.deepEqual(f.contexts.forTarget('tab'), []);
  f.contexts.dispose();
});
test('disconnect rejects an already-pending native reply without restoring owner state', async () => {
  const f = fixture();
  let resolve!: (value: any) => void;
  f.child._runOyaRuntime = () =>
    new Promise((r) => {
      resolve = r;
    });
  const pending = f.contexts.current(f.page, 'tab', f.child as any);
  f.contexts.dispose();
  resolve({ context: 'late' });
  await assert.rejects(pending, /closed/);
  assert.deepEqual(f.contexts.forTarget('tab'), []);
});

test('isolated creation after disconnect closes the exact late native token and key', async () => {
  const f = fixture(),
    calls: any[] = [];
  let allocated!: () => void, reply!: (value: any) => void;
  const started = new Promise<void>((r) => {
    allocated = r;
  });
  f.top._runOyaRuntime = (async (...args: any[]) => {
    calls.push(args);
    if (args[2] === 'isolatedContext') {
      allocated();
      return new Promise((r) => {
        reply = r;
      });
    }
    return args[2] === 'context' ? { context: 'top' } : {};
  }) as any;
  const pending = f.contexts.create(f.page, 'tab', { frameId: 'tab', worldName: 'private' });
  await started;
  f.contexts.dispose();
  reply({ context: 'late-world' });
  await assert.rejects(pending, /closed/);
  const closed = calls.find((c) => c[2] === 'close' && c[1] === 'late-world');
  assert.ok(closed);
  assert.equal(closed[3].world, calls.find((c) => c[2] === 'isolatedContext')[3].world);
  assert.deepEqual(f.contexts.forTarget('tab'), []);
});

test('world creation on a removed frame revokes the native allocation without announcing it', async () => {
  const f = fixture(),
    calls: any[] = [];
  f.child._runOyaRuntime = (async (...args: any[]) => {
    calls.push(args);
    if (args[2] === 'isolatedContext') {
      f.top.framesInSubtree = [f.top];
      return { context: 'removed-world' };
    }
    return { context: 'child' };
  }) as any;
  let changes = 0;
  f.contexts.subscribe(() => {
    changes++;
  });
  await assert.rejects(f.contexts.create(f.page, 'tab', { frameId: 'child-frame', worldName: 'private' }), /foreign/);
  assert.equal(changes, 0);
  assert.ok(calls.some((c) => c[2] === 'close' && c[1] === 'removed-world'));
  assert.ok(!f.contexts.forTarget('tab').some((c) => c.world));
  f.contexts.dispose();
});

test('disconnect releases both main and isolated tokens using their exact native world keys', async () => {
  const f = fixture(),
    calls: any[] = [];
  f.top._runOyaRuntime = (async (...args: any[]) => {
    calls.push(args);
    if (args[2] === 'isolatedContext') return { context: 'owned-world' };
    if (args[2] === 'context') return { context: args[3].world ? 'owned-world' : 'top' };
    return {};
  }) as any;
  const context = await f.contexts.create(f.page, 'tab', { frameId: 'tab', worldName: 'owned' });
  f.contexts.dispose();
  const closed = calls.filter((c) => c[2] === 'close');
  assert.equal(closed.length, 2);
  assert.deepEqual(
    closed.map((c) => [c[1], c[3].world]),
    [
      ['top', undefined],
      ['owned-world', context.world],
    ],
  );
  assert.ok(closed.every((c) => c[0] === f.contexts.owner));
  await assert.rejects(f.contexts.resolve(f.page, 'tab', context), /closed/);
});
