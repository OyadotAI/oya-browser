/** Native backend admission must survive asynchronous tab protection and a human takeover. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { AppNativeBackend, startAppNativeCdp } from '../../../../src/main/app/native-cdp.ts';

/** A published native tab can still have protection settlement awaiting its final microtask. */
function fixture(nativeBrowsing = true) {
  let finish!: () => void;
  const inserted: string[] = [];
  const setup = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const control = { busy: false, localHeld: false, connected: true, state: { mode: 'agent' } };
  const tab = {
    id: 1,
    home: false,
    protection: 'protected',
    setup,
    view: {
      webContents: {
        session: {},
        isDestroyed: () => false,
        getURL: () => 'https://example.test',
        getTitle: () => 'Test',
        _insertTextOya: async (text: string) => {
          inserted.push(text);
        },
      },
    },
  };
  const deps: any = { nativeBrowsing, windows: { allTabs: () => [tab] }, control };
  const backend = new AppNativeBackend(deps);
  return {
    backend,
    deps,
    tab,
    control,
    inserted,
    finish,
    target: nativeBrowsing ? backend.targets()[0].targetId : 'legacy-target',
  };
}

it('refuses native input when human control changes while tab protection is awaiting settlement', async () => {
  const f = fixture();
  const pending = f.backend.execute(f.target, 'input:text', { text: 'must not type' });
  f.control.localHeld = true;
  f.finish();
  await assert.rejects(pending, /control is unavailable/i);
  assert.deepEqual(f.inserted, []);
  f.backend.dispose();
});

/** The real backend receives a deferred isolated layout operation on its exact protected tab. */
function wheelFixture() {
  const f = fixture();
  let ready!: () => void, start!: () => void;
  const reading = new Promise<void>((resolve) => {
    start = resolve;
  });
  const layout = new Promise<void>((resolve) => {
    ready = resolve;
  });
  const events: unknown[] = [];
  const contents = Object.assign(new EventEmitter(), f.tab.view.webContents, {
    mainFrame: {
      detached: false,
      _executeJavaScriptInOyaWorld: () => {
        start();
        return layout;
      },
    },
    getZoomFactor: () => 1,
    sendInputEvent: (event: unknown) => {
      events.push(event);
    },
  });
  f.tab.view.webContents = contents;
  f.finish();
  const pending = f.backend.execute(f.target, 'input:pointer', {
    type: 'mouseWheel',
    x: 150,
    y: 150,
    deltaX: 0,
    deltaY: 120,
  });
  return { ...f, reading, ready, pending, events };
}

for (const mode of ['localHeld', 'busy', 'human']) {
  it(`refuses a pending wheel when control changes through ${mode} during layout readiness`, async () => {
    const f = wheelFixture();
    await f.reading;
    if (mode === 'human') f.control.state.mode = 'human';
    else f.control[mode] = true;
    f.ready();
    await assert.rejects(f.pending, /control is unavailable/i);
    assert.deepEqual(f.events, []);
    f.backend.dispose();
  });
}

it('sends the native wheel only while its original protected tab remains authorized', async () => {
  const f = wheelFixture();
  await f.reading;
  f.ready();
  await f.pending;
  assert.equal(f.events.length, 1);
  f.backend.dispose();
});

it('does not send a saved wheel when the protected tab is retired during readiness', async () => {
  const f = wheelFixture();
  await f.reading;
  f.tab.protection = 'failed';
  f.ready();
  await assert.rejects(f.pending, /target is unavailable/i);
  assert.deepEqual(f.events, []);
  f.backend.dispose();
});

it('does not send or retarget a saved wheel when the tab replaces its native view', async () => {
  const f = wheelFixture();
  await f.reading;
  f.tab.view = { ...f.tab.view };
  f.ready();
  await assert.rejects(f.pending, /input target changed/i);
  assert.deepEqual(f.events, []);
  f.backend.dispose();
});

it('executes native input when protection settles under the same agent owner', async () => {
  const f = fixture();
  const pending = f.backend.execute(f.target, 'input:text', { text: 'native' });
  f.finish();
  await pending;
  assert.deepEqual(f.inserted, ['native']);
  f.backend.dispose();
});

it('legacy mode refuses native adapter actions before inherited tab and popup lifecycles can run', async () => {
  const f = fixture(false);
  f.finish();
  for (const action of ['input:text', 'runtime:evaluate', 'navigate']) {
    await assert.rejects(
      f.backend.execute(f.target, action, {
        text: 'blocked',
        expression: 'window.open()',
        url: 'https://example.test',
      }),
      /native browsing/i,
    );
  }
  await assert.rejects(f.backend.open('https://example.test'), /native browsing/i);
  await assert.rejects(f.backend.close(f.target), /native browsing/i);
  await assert.rejects(
    f.backend.manage('context:create', {}, () => {}),
    /native browsing/i,
  );
  assert.throws(() => f.backend.subscribe(f.target, 'Page', () => {}), /native browsing/i);
  assert.deepEqual(f.inserted, []);
  f.backend.dispose();
});

it('configured compatibility listener refuses legacy mode before allocating a server', () => {
  assert.throws(
    () => startAppNativeCdp({ nativeBrowsing: false } as any, { port: 12345, token: 'x'.repeat(32) }),
    /native browsing/i,
  );
});

it('native operations wait for original text restoration on their exact tab', async () => {
  const f = fixture();
  let release!: () => void;
  f.deps.translation = {
    released: async (view: unknown) => {
      assert.equal(view, f.tab.view);
      await new Promise<void>((resolve) => {
        release = resolve;
      });
    },
  };
  const pending = f.backend.execute(f.target, 'input:text', { text: 'after restoration' });
  f.finish();
  await new Promise(setImmediate);
  assert.deepEqual(f.inserted, []);
  release();
  await pending;
  assert.deepEqual(f.inserted, ['after restoration']);
  f.backend.dispose();
});

it('external passthrough cannot read a document whose original text failed to restore', async () => {
  const f = fixture();
  let debuggerRead = false;
  Object.defineProperty(f.tab.view.webContents, 'debugger', {
    get() {
      debuggerRead = true;
      throw Error('must not reach debugger');
    },
  });
  f.deps.translation = {
    released: async () => {
      throw Error('original page unavailable');
    },
  };
  const pending = f.backend.passthrough(f.target, 'Runtime.evaluate', { expression: 'document.body.innerText' });
  f.finish();
  await assert.rejects(pending, /original page unavailable/);
  assert.equal(debuggerRead, false);
  f.backend.dispose();
});
