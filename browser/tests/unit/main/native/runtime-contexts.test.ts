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
