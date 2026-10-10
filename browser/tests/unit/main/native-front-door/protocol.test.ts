/** The compatibility boundary dispatches only explicit native capabilities within one connection’s target scope. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mock } from 'node:test';
import { NativeProtocol } from '../../../../src/main/native-front-door/protocol.ts';
test('JPEG quality is forwarded to native capture and invalid quality never reaches the backend', async () => {
  const f = fixture();
  const execute = mock.method(f.backend, 'execute', async () => ({
    /** Encoded fixture pixels returned by the native backend. */
    screenshot: 'data:image/jpeg;base64,SlBFRw==',
  }));
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as {
    /** Connection-owned target session returned by attachment. */
    sessionId: string;
  };
  assert.deepEqual(await f.run('Page.captureScreenshot', { format: 'jpeg', quality: 40 }, sessionId), {
    data: 'SlBFRw==',
  });
  assert.deepEqual(execute.mock.calls[0].arguments, ['a', 'screenshot', { format: 'jpeg', quality: 40 }]);
  for (const quality of [-1, 101, 1.5, '40', null])
    await assert.rejects(f.run('Page.captureScreenshot', { format: 'jpeg', quality }, sessionId), /quality/);
  await assert.rejects(f.run('Page.captureScreenshot', { format: 'png', quality: 40 }, sessionId), /quality/);
  assert.equal(execute.mock.callCount(), 1);
});
/** Mutable fake capability adapter, without a protocol transport. */
function fixture() {
  const calls: unknown[] = [],
    events: any[] = [];
  let targets = [{ targetId: 'a', type: 'page' as const, title: 'A', url: 'https://example.test' }];
  let sink: any,
    stops = 0;
  const backend = {
    targets: () => targets,
    open: async (url: string) => {
      calls.push(['open', url]);
      return 'new';
    },
    close: async (id: string) => {
      calls.push(['close', id]);
    },
    execute: async (...args: unknown[]) => {
      calls.push(args);
      return { root: { nodeId: 1 } };
    },
    subscribe: (_target: string, _domain: string, emit: any) => {
      sink = emit;
      return () => {
        stops++;
      };
    },
  };
  const protocol = new NativeProtocol(backend, undefined, (event) => events.push(event));
  const run = (method: string, params: any = {}, sessionId?: string) =>
    protocol.dispatch({ id: 1, method, params, sessionId });
  return {
    protocol,
    backend,
    run,
    calls,
    events,
    emit: () => sink('Log.entryAdded', { entry: {} }),
    stops: () => stops,
    revoke: () => {
      targets = [];
    },
  };
}
test('DOM reads use native methods on the exact attached target', async () => {
  const f = fixture();
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  assert.deepEqual(await f.run('DOM.getDocument', { depth: 2 }, sessionId), { root: { nodeId: 1 } });
  assert.deepEqual(f.calls, [['a', 'dom:document', { depth: 2 }]]);
});
test('foreign sessions cannot address another connection’s target', async () => {
  const f = fixture(),
    other = new NativeProtocol(f.backend);
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  await assert.rejects(other.dispatch({ id: 1, method: 'DOM.getDocument', params: {}, sessionId }), /outside/);
  f.revoke();
  await assert.rejects(f.run('DOM.getDocument', {}, sessionId), /outside/);
  assert.equal(f.calls.length, 0);
});
test('unsupported contexts, shadow semantics and unsafe DOM ids never reach native code', async () => {
  const f = fixture();
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  for (const params of [{ depth: -1 }, { depth: 33 }, { depth: '1' }, { pierce: true }, { frameId: 'other' }])
    await assert.rejects(f.run('DOM.getDocument', params, sessionId));
  for (const params of [{ nodeId: 0, selector: 'input' }, { nodeId: 1 }, { backendNodeId: 1, selector: 'input' }])
    await assert.rejects(f.run('DOM.querySelector', params, sessionId));
  assert.equal(f.calls.length, 0);
});
test('unimplemented domains fail explicitly without invoking backend operations', async () => {
  const f = fixture();
  for (const method of [
    'Runtime.runIfWaitingForDebugger',
    'Network.searchInResponseBody',
    'Debugger.enable',
    '__proto__',
  ])
    await assert.rejects(f.run(method), /Unsupported native capability/);
  assert.equal(f.calls.length, 0);
});
test('pinned page sockets cannot manage other targets', async () => {
  const f = fixture(),
    direct = new NativeProtocol(f.backend, 'a');
  await assert.rejects(
    direct.dispatch({ id: 1, method: 'Target.createTarget', params: { url: 'https://example.test' } }),
    /browser endpoint/,
  );
  assert.equal(f.calls.length, 0);
});
test('native log events retain their session and unsubscribe on detach and disconnect', async () => {
  const f = fixture();
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  await f.run('Log.enable', {}, sessionId);
  f.emit();
  assert.deepEqual(f.events, [{ method: 'Log.entryAdded', params: { entry: {} }, sessionId }]);
  await f.run('Log.enable', {}, sessionId);
  assert.equal(f.stops(), 0);
  await f.run('Target.detachFromTarget', { sessionId });
  assert.equal(f.stops(), 1);
  f.protocol.dispose();
  assert.equal(f.stops(), 1);
});
test('local and executable URLs and semantic options never reach native navigation', async () => {
  const f = fixture();
  for (const url of ['file:///private/file', 'javascript:alert(1)'])
    await assert.rejects(f.run('Target.createTarget', { url }));
  await assert.rejects(f.run('Target.createTarget', { url: 'https://example.test', newWindow: true }));
  assert.equal(f.calls.length, 0);
});
test('page readiness subscriptions coexist with logs and disabling one preserves the other', async () => {
  const f = fixture();
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  await f.run('Log.enable', {}, sessionId);
  await f.run('Page.enable', {}, sessionId);
  await f.run('Page.enable', {}, sessionId);
  assert.equal(f.stops(), 0);
  await f.run('Page.disable', {}, sessionId);
  assert.equal(f.stops(), 1);
  f.protocol.dispose();
  assert.equal(f.stops(), 2);
});
test('native reload, stop, focus, scroll and text commands preserve exact session routing', async () => {
  const f = fixture();
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  for (const [method, action, params] of [
    ['Page.reload', 'page:reload', { ignoreCache: true }],
    ['Page.stopLoading', 'page:stop', {}],
    ['DOM.focus', 'dom:focus', { nodeId: 1 }],
    ['DOM.scrollIntoViewIfNeeded', 'dom:scroll', { nodeId: 1 }],
    ['Input.insertText', 'input:text', { text: '日本語 🙂' }],
  ] as const) {
    await f.run(method, params, sessionId);
    assert.deepEqual(f.calls.at(-1), ['a', action, params]);
  }
});
test('unsupported input, focus and reload options fail before native state changes', async () => {
  const f = fixture();
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  for (const [method, params] of [
    ['Page.reload', { ignoreCache: 'true' }],
    ['Page.reload', { scriptToEvaluateOnLoad: 'x' }],
    ['DOM.focus', { objectId: 'foreign' }],
    ['DOM.scrollIntoViewIfNeeded', { nodeId: 1, rect: {} }],
    ['Input.insertText', {}],
    ['Input.insertText', { text: 2 }],
  ] as const)
    await assert.rejects(f.run(method, params, sessionId));
  assert.equal(f.calls.length, 0);
});
test('native history commands preserve exact target scope and require a snapshot plus numeric index', async () => {
  const f = fixture();
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  await f.run('Oya.getNavigationHistory', {}, sessionId);
  await f.run('Oya.navigateToHistoryEntry', { snapshot: 'opaque', index: 0 }, sessionId);
  assert.deepEqual(f.calls, [
    ['a', 'history:read', {}],
    ['a', 'history:navigate', { snapshot: 'opaque', index: 0 }],
  ]);
  for (const params of [{}, { snapshot: 'x', index: '0' }, { snapshot: '', index: 0 }, { snapshot: 'x', index: -1 }])
    await assert.rejects(f.run('Oya.navigateToHistoryEntry', params, sessionId), /snapshot|index/);
  await assert.rejects(f.run('Oya.getNavigationHistory', { extra: true }, sessionId), /Unsupported/);
  f.revoke();
  await assert.rejects(f.run('Oya.getNavigationHistory', {}, sessionId), /outside/);
});
test('capability discovery describes actual adapter methods and validation without backend access', async () => {
  const f = fixture();
  const manifest = (await f.run('Oya.getCapabilities')) as any;
  assert.equal(manifest.compatibility, 'partial');
  assert.equal(manifest.availability, 'checked-at-execution');
  assert.equal(new Set(manifest.methods.map((m: any) => m.method)).size, manifest.methods.length);
  assert.deepEqual(
    manifest.methods.find((m: any) => m.method === 'Oya.navigateToHistoryEntry'),
    {
      method: 'Oya.navigateToHistoryEntry',
      allowedParameters: ['snapshot', 'index'],
      scope: 'target',
    },
  );
  assert.equal(manifest.methods.find((m: any) => m.method === 'Storage.getCookies').scope, 'browser');
  assert.equal(manifest.methods.find((m: any) => m.method === 'Oya.getCapabilities').scope, 'connection');
  assert.ok(
    !manifest.methods.some((m: any) =>
      ['Debugger.enable', 'Network.enable', 'Fetch.fulfillRequest'].includes(m.method),
    ),
  );
  manifest.methods.find((m: any) => m.method === 'Fetch.enable').allowedParameters.push('unsafe');
  const fresh = (await f.run('Oya.getCapabilities')) as any;
  assert.ok(!fresh.methods.find((m: any) => m.method === 'Fetch.enable').allowedParameters.includes('unsafe'));
  await assert.rejects(f.run('Oya.getCapabilities', { verbose: true }), /Unsupported/);
  assert.deepEqual(f.calls, []);
});
