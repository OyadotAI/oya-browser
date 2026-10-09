/** Native downloads enforce exact context, control ownership and safe, explicit save destinations. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { mkdtempSync, rmSync, existsSync } from 'node:fs';
import { realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { NativeDownloads } from '../../../../src/main/native-downloads/index.ts';
/** Native item seam records cancellation and native save selection. */
function fixture() {
  const session = new EventEmitter() as any,
    contents = { session } as any;
  let canceled = 0,
    path = '',
    allowed = true;
  const item = Object.assign(new EventEmitter(), {
    cancel: () => {
      canceled++;
    },
    setSavePath: (p: string) => {
      path = p;
    },
    getURL: () => 'https://example.test/download',
    getFilename: () => '../../untrusted',
    getTotalBytes: () => 100,
    getReceivedBytes: () => 50,
  });
  const events: any[] = [],
    downloads = new NativeDownloads({ allowed: () => allowed, target: () => 'exact' });
  downloads.install(session);
  return {
    session,
    contents,
    item,
    downloads,
    events,
    canceled: () => canceled,
    path: () => path,
    human: () => {
      allowed = false;
    },
    emit: (...e: any[]) => events.push(e),
    start: () => session.emit('will-download', {}, item, contents),
  };
}
test('private contexts deny downloads before an explicit owned destination is configured', () => {
  const f = fixture();
  f.start();
  assert.equal(f.canceled(), 1);
  assert.equal(f.path(), '');
  f.downloads.remove(f.session);
  assert.equal(f.session.listenerCount('will-download'), 0);
});
test('GUID destinations ignore hostile filenames and cancellation refuses foreign contexts', () => {
  const root = mkdtempSync(join(realpathSync(tmpdir()), 'oya-download-test-'));
  const f = fixture();
  try {
    f.downloads.set(f.session, { behavior: 'allowAndName', downloadPath: root, eventsEnabled: true }, f.emit);
    f.start();
    const guid = f.events[0][1].guid;
    assert.equal(f.path(), join(root, guid));
    assert.equal(existsSync(f.path()), true);
    assert.throws(() => f.downloads.cancel({} as any, guid), /foreign/);
    f.item.emit('updated', {}, 'progressing');
    assert.equal(f.events[1][1].receivedBytes, 50);
    f.human();
    f.item.emit('updated', {}, 'progressing');
    assert.equal(f.canceled(), 1);
    assert.equal(f.events.length, 2);
    f.downloads.remove(f.session);
    assert.throws(() => f.downloads.cancel(f.session, guid), /foreign/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
test('context cleanup removes only its observer and never announces canceled private activity', () => {
  const f = fixture(),
    other = () => {};
  f.session.on('will-download', other);
  f.downloads.remove(f.session);
  assert.deepEqual(f.session.listeners('will-download'), [other]);
  assert.equal(f.events.length, 0);
});
