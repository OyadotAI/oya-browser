/** Native isolated-world behavior, including document replacement and fatal debugger access. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { World } from '../../../../src/main/native/index.ts';
/** An isolated renderer global replaced by each navigation. */
function fixture() {
  let context = vm.createContext({});
  const calls = [];
  const webContents = {
    isDestroyed: () => false,
    get debugger() {
      throw new Error('CDP is forbidden');
    },
    async executeJavaScriptInIsolatedWorld(id, scripts) {
      calls.push({ id, scripts });
      return vm.runInContext(scripts[0].code, context);
    },
  };
  return {
    view: { webContents },
    calls,
    navigate: () => {
      context = vm.createContext({});
    },
  };
}
/** Analyzer fixture has a visible installation count only inside its isolated global. */
const source =
  'globalThis.loads=(globalThis.loads||0)+1; globalThis.attr="__OYA_ATTR__"; globalThis.record=__OYA_RECORD__;';
/** Native-only construction never accepts a protocol sender. */
const world = () => new World({ analyzerScript: source, worldName: 'test-world' });
it('initializes once per document without debugger access', async () => {
  const { view, calls } = fixture();
  const service = world();
  await service.ensure(view);
  assert.equal(await service.evaluate(view, 'loads'), 1);
  assert.equal(await service.evaluate(view, 'record'), false);
  assert.match(await service.evaluate(view, 'attr'), /^data-[a-f0-9]{8}$/);
  assert.ok(calls.every((call) => call.id > 999));
});
it('reinstalls after navigation and when explicitly forced', async () => {
  const { view, navigate } = fixture();
  const service = world();
  await service.ensure(view);
  await service.ensure(view, { force: true });
  assert.equal(await service.evaluate(view, 'loads'), 2);
  navigate();
  assert.equal(await service.evaluate(view, 'loads'), 1);
});
it('returns asynchronous values and never retries ordinary script failures', async () => {
  const { view } = fixture();
  const service = world();
  assert.equal(await service.evaluate(view, 'Promise.resolve(42)'), 42);
  await assert.rejects(
    service.evaluate(view, 'loads++; throw new Error("application failure")'),
    /application failure/,
  );
  assert.equal(await service.evaluate(view, 'loads'), 2);
});
it('rejects a destroyed renderer before running code', async () => {
  const { view, calls } = fixture();
  view.webContents.isDestroyed = () => true;
  await assert.rejects(world().ensure(view), /destroyed/);
  assert.equal(calls.length, 0);
});

it('retries a disposed native execution context once, without retrying indefinitely', async () => {
  const { view } = fixture();
  const execute = view.webContents.executeJavaScriptInIsolatedWorld;
  let attempts = 0;
  view.webContents.executeJavaScriptInIsolatedWorld = async (id, scripts) => {
    if (scripts[0].code.endsWith('\n42') && ++attempts === 1) throw new Error('Execution context was destroyed');
    return execute(id, scripts);
  };
  assert.equal(await world().evaluate(view, '42'), 42);
  assert.equal(attempts, 2);
});
it('stops after the one permitted native context retry', async () => {
  const { view } = fixture();
  const execute = view.webContents.executeJavaScriptInIsolatedWorld;
  let attempts = 0;
  view.webContents.executeJavaScriptInIsolatedWorld = async (id, scripts) => {
    if (scripts[0].code.endsWith('\n42')) {
      attempts++;
      throw new Error('Render frame was disposed');
    }
    return execute(id, scripts);
  };
  await assert.rejects(world().evaluate(view, '42'), /disposed/);
  assert.equal(attempts, 2);
});

it('uses the patched engine frame API without waiting for page resource completion', async () => {
  const { view } = fixture();
  const context = vm.createContext({});
  view.webContents.mainFrame = {
    detached: false,
    _executeJavaScriptInOyaWorld: async (code) => vm.runInContext(code, context),
  };
  view.webContents.executeJavaScriptInIsolatedWorld = async () => assert.fail('must not wait for did-stop-loading');
  assert.equal(await world().evaluate(view, 'loads'), 1);
});
it('does not fall back when the native engine frame operation fails', async () => {
  const { view } = fixture();
  view.webContents.mainFrame = {
    detached: false,
    _executeJavaScriptInOyaWorld: async () => {
      throw Error('native failure');
    },
  };
  view.webContents.executeJavaScriptInIsolatedWorld = async () => assert.fail('must not hide native failure');
  await assert.rejects(world().evaluate(view, '42'), /native failure/);
});

it('initialization and analysis use one execution so navigation cannot split the two calls', async () => {
  const { view, calls, navigate } = fixture();
  const execute = view.webContents.executeJavaScriptInIsolatedWorld;
  view.webContents.executeJavaScriptInIsolatedWorld = async (id, scripts) => {
    const result = await execute(id, scripts);
    navigate();
    return result;
  };
  assert.equal(await world().evaluate(view, 'loads'), 1);
  assert.equal(calls.length, 1);
});
