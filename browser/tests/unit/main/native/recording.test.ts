/** Native recording admission is scoped to an owned page and the current recording epoch. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { NativeRecordingInbox } from '../../../../src/main/native/index.ts';
import { NATIVE_RECORDING } from '../../../../src/shared/native-recording.ts';
/** Test only the native event seam; no browser or network is involved. */
function fixture() {
  const page = Object.assign(new EventEmitter(), { isDestroyed: () => false });
  const received: unknown[] = [];
  const inbox = new NativeRecordingInbox(page as never, (message) => received.push(message));
  const frame = {
    detached: false,
    url: 'https://original.test',
    async _executeJavaScriptInOyaWorld() {
      return 'document';
    },
  };
  Object.assign(page, { mainFrame: frame });
  const emit = (epoch: string, payload: unknown = '{}', sender: unknown = page, channel = NATIVE_RECORDING.CHANNEL) =>
    page.emit('ipc-message', { sender, senderFrame: frame }, channel, epoch, payload, 'document');
  return { page, received, inbox, frame, emit };
}
test('attributes messages to the browser supplied frame', async () => {
  const f = fixture();
  const epoch = f.inbox.start();
  const document = await f.inbox.authorize(f.frame as never);
  f.emit(epoch);
  assert.deepEqual(f.received, [{ frame: f.frame, payload: '{}', document }]);
});
test('rejects unrelated channels, senders, epochs and oversized batches', async () => {
  const f = fixture();
  const epoch = f.inbox.start();
  await f.inbox.authorize(f.frame as never);
  f.emit('old');
  f.emit(epoch, '{}', {});
  f.emit(epoch, '{}', f.page, 'other' as never);
  f.emit(epoch, {});
  f.emit(epoch, 'x'.repeat(NATIVE_RECORDING.MAX_PAYLOAD_CHARS + 1));
  assert.deepEqual(f.received, []);
});
test('restarting rotates the epoch and does not duplicate subscriptions', async () => {
  const f = fixture();
  const first = f.inbox.start();
  assert.equal(f.inbox.start(), first);
  f.inbox.stop();
  f.emit(first);
  const second = f.inbox.start();
  assert.notEqual(second, first);
  f.emit(first);
  await f.inbox.authorize(f.frame as never);
  f.emit(second);
  assert.equal(f.received.length, 1);
  assert.equal(f.page.listenerCount('ipc-message'), 1);
});
test('destruction removes listeners and refuses another start', () => {
  const f = fixture();
  f.inbox.start();
  f.page.isDestroyed = () => true;
  f.page.emit('destroyed');
  assert.equal(f.page.listenerCount('ipc-message'), 0);
  assert.equal(f.page.listenerCount('destroyed'), 0);
  assert.throws(() => f.inbox.start(), /destroyed/);
});

test('unarmed documents cannot deliver even with the current epoch', () => {
  const f = fixture();
  f.emit(f.inbox.start());
  assert.deepEqual(f.received, []);
});
test('authorized unload batches retain the original immutable attribution', async () => {
  const f = fixture();
  const epoch = f.inbox.start();
  const document = await f.inbox.authorize(f.frame as never);
  f.frame.url = 'https://replacement.test';
  f.frame.detached = true;
  f.emit(epoch);
  assert.equal(document.url, 'https://original.test');
  assert.ok(Object.isFrozen(document));
  assert.ok(Object.isFrozen(document.frames));
  assert.equal(f.received.length, 1);
});
test('another native frame cannot reuse a document authorization', async () => {
  const f = fixture();
  const epoch = f.inbox.start();
  await f.inbox.authorize(f.frame as never);
  f.page.emit('ipc-message', { sender: f.page, senderFrame: {} }, NATIVE_RECORDING.CHANNEL, epoch, '{}', 'document');
  assert.deepEqual(f.received, []);
});
test('navigation during authorization rejects rather than mixing document metadata', async () => {
  const f = fixture();
  f.inbox.start();
  let reads = 0;
  f.frame._executeJavaScriptInOyaWorld = async () => String(++reads);
  await assert.rejects(f.inbox.authorize(f.frame as never), /document changed/);
});
test('stopping during authorization cannot reauthorize an old recording', async () => {
  const f = fixture();
  f.inbox.start();
  f.frame._executeJavaScriptInOyaWorld = async () => {
    f.inbox.stop();
    f.inbox.start();
    return 'document';
  };
  await assert.rejects(f.inbox.authorize(f.frame as never), /Recording changed/);
});
test('restarting requires document authorization again', async () => {
  const f = fixture();
  f.inbox.start();
  await f.inbox.authorize(f.frame as never);
  f.inbox.stop();
  f.emit(f.inbox.start());
  assert.deepEqual(f.received, []);
});
test('stopped inbox refuses to authorize a document', async () => {
  const f = fixture();
  await assert.rejects(f.inbox.authorize(f.frame as never), /stopped/);
});
test('missing isolated preload identity is an explicit capability failure', async () => {
  const f = fixture();
  f.inbox.start();
  f.frame._executeJavaScriptInOyaWorld = async () => '';
  await assert.rejects(f.inbox.authorize(f.frame as never), /identity is unavailable/);
});

test('re-arming an authorized document preserves its original capture metadata', async () => {
  const f = fixture();
  f.inbox.start();
  const original = await f.inbox.authorize(f.frame as never);
  f.frame.url = 'https://original.test/changed-by-history-api';
  assert.equal(await f.inbox.authorize(f.frame as never), original);
});
test('a document identity collision cannot replace an existing frame authorization', async () => {
  const f = fixture();
  const epoch = f.inbox.start();
  await f.inbox.authorize(f.frame as never);
  const replacement = { ...f.frame };
  Object.assign(f.page, { mainFrame: replacement });
  await assert.rejects(f.inbox.authorize(replacement as never), /collision/);
  f.emit(epoch);
  assert.equal(f.received.length, 1);
});
test('an unavailable native sender frame cannot deliver an authorized document', async () => {
  const f = fixture();
  const epoch = f.inbox.start();
  await f.inbox.authorize(f.frame as never);
  f.page.emit('ipc-message', { sender: f.page, senderFrame: null }, NATIVE_RECORDING.CHANNEL, epoch, '{}', 'document');
  assert.deepEqual(f.received, []);
});

test('bounds retained document authorizations without evicting final-unload attribution', async () => {
  const f = fixture();
  const epoch = f.inbox.start();
  await f.inbox.authorize(f.frame as never);
  for (let index = 1; index < NATIVE_RECORDING.MAX_DOCUMENTS; index++) {
    f.frame._executeJavaScriptInOyaWorld = async () => String(index);
    await f.inbox.authorize(f.frame as never);
  }
  f.frame._executeJavaScriptInOyaWorld = async () => 'overflow';
  await assert.rejects(f.inbox.authorize(f.frame as never), /document limit exceeded/);
  f.emit(epoch);
  assert.equal(f.received.length, 1);
  f.inbox.stop();
  f.inbox.start();
  await f.inbox.authorize(f.frame as never);
});
