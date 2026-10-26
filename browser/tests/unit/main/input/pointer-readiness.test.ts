/** Readiness cannot move native input across document retirement, renderer failure or control revocation. */
import { EventEmitter } from 'node:events';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withPointerReadiness } from '../../../../src/main/input/index.ts';

/** One deferred layout read exposes exactly the admission interval tested here. */
function fixture() {
  let resolve!: (value?: unknown) => void, reject!: (error: Error) => void;
  const layout = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  const frame = { detached: false };
  const contents = Object.assign(new EventEmitter(), { mainFrame: frame, isDestroyed: () => false });
  const order: string[] = [];
  const page = { webContents: contents } as any;
  const read = (actual: unknown) => {
    assert.equal(actual, frame);
    order.push('read');
    return layout;
  };
  const send = () => {
    order.push('send');
  };
  return { page, contents, frame, order, read, send, resolve, reject };
}

test('readiness finishes on the captured frame before caller admission and the native send', async () => {
  const f = fixture();
  const pending = withPointerReadiness(
    f.page,
    f.read,
    () => {
      f.order.push('guard');
    },
    f.send,
  );
  assert.deepEqual(f.order, ['read']);
  f.resolve();
  await pending;
  assert.deepEqual(f.order, ['read', 'guard', 'send']);
  assert.deepEqual(f.contents.eventNames(), []);
});

test('a revoked caller cannot send a wheel after its layout read resolves', async () => {
  const f = fixture();
  let allowed = true;
  const pending = withPointerReadiness(
    f.page,
    f.read,
    () => {
      if (!allowed) throw Error('control revoked');
    },
    f.send,
  );
  allowed = false;
  f.resolve();
  await assert.rejects(pending, /control revoked/);
  assert.deepEqual(f.order, ['read']);
  assert.deepEqual(f.contents.eventNames(), []);
});

for (const event of ['did-navigate', 'render-process-gone', 'destroyed']) {
  test(`document retirement through ${event} rejects without dispatch and cleans listeners`, async () => {
    const f = fixture();
    const pending = withPointerReadiness(f.page, f.read, () => {}, f.send);
    f.contents.emit(event);
    f.resolve();
    await assert.rejects(pending, /document changed/);
    assert.deepEqual(f.order, ['read']);
    assert.deepEqual(f.contents.eventNames(), []);
  });
}

test('starting a replacement navigation rejects even if the frame wrapper and URL are reused', async () => {
  const f = fixture();
  const pending = withPointerReadiness(f.page, f.read, () => {}, f.send);
  f.contents.emit('did-start-navigation', {}, 'https://same.test/', false, true);
  f.resolve();
  await assert.rejects(pending, /document changed/);
  assert.deepEqual(f.order, ['read']);
});

test('same-document and unrelated child-frame navigation do not replace the intended document', async () => {
  const f = fixture();
  const pending = withPointerReadiness(f.page, f.read, () => {}, f.send);
  f.contents.emit('did-start-navigation', {}, 'https://same.test/#hash', true, true);
  f.contents.emit('did-start-navigation', {}, 'https://child.test/', false, false);
  f.resolve();
  await pending;
  assert.deepEqual(f.order, ['read', 'send']);
});

test('detached frames and replacement main frames cannot receive the saved wheel', async () => {
  for (const replace of [false, true]) {
    const f = fixture();
    const pending = withPointerReadiness(f.page, f.read, () => {}, f.send);
    if (replace) f.contents.mainFrame = { detached: false };
    else f.frame.detached = true;
    f.resolve();
    await assert.rejects(pending, /document changed/);
    assert.deepEqual(f.order, ['read']);
    assert.deepEqual(f.contents.eventNames(), []);
  }
});

test('renderer read errors release listeners and preserve the original failure', async () => {
  const f = fixture(),
    error = Error('renderer failed');
  const pending = withPointerReadiness(f.page, f.read, () => {}, f.send);
  f.reject(error);
  await assert.rejects(pending, (actual) => actual === error);
  assert.deepEqual(f.order, ['read']);
  assert.deepEqual(f.contents.eventNames(), []);
});

test('a native send failure also releases every transient lifetime listener', async () => {
  const f = fixture();
  const pending = withPointerReadiness(
    f.page,
    f.read,
    () => {},
    () => {
      throw Error('native send failed');
    },
  );
  f.resolve();
  await assert.rejects(pending, /native send failed/);
  assert.deepEqual(f.contents.eventNames(), []);
});

test('document lifetime listeners remain installed through the actual synchronous send', async () => {
  const f = fixture();
  const pending = withPointerReadiness(
    f.page,
    f.read,
    () => {},
    () => {
      assert.equal(f.contents.listenerCount('did-start-navigation'), 1);
      assert.equal(f.contents.listenerCount('destroyed'), 1);
      f.send();
    },
  );
  f.resolve();
  await pending;
  assert.deepEqual(f.order, ['read', 'send']);
  assert.deepEqual(f.contents.eventNames(), []);
});

test('navigation during final caller admission cannot cross into the native send', async () => {
  const f = fixture();
  const pending = withPointerReadiness(
    f.page,
    f.read,
    () => {
      f.contents.emit('did-start-navigation', {}, 'https://replacement.test/', false, true);
    },
    f.send,
  );
  f.resolve();
  await assert.rejects(pending, /document changed/);
  assert.deepEqual(f.order, ['read']);
  assert.deepEqual(f.contents.eventNames(), []);
});
