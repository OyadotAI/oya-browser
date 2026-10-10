/** Native backend admission must survive asynchronous tab protection and a human takeover. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
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
  const backend = new AppNativeBackend({ nativeBrowsing, windows: { allTabs: () => [tab] }, control } as any);
  return {
    backend,
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
