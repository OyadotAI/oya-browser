/** The production storage lifecycle on a real native partition, with synthetic login data and no debugger. */
const assert = require('node:assert/strict');
const { NativeStorageSync } = require('../../src/main/sync/native-storage.ts');
/** Restore before navigation, retain offline logout, and fence a retired persona's observer. */
module.exports = async function storageSyncChecks({ jar, win, origin, mutation }) {
  const sent = [];
  let online = false;
  const sync = new NativeStorageSync({
    session: jar,
    ready: () => online,
    send: (message) => {
      sent.push(message);
      return true;
    },
  });
  try {
    await sync.initialize({ [origin]: { account: 'Restored natively' } });
    await win.webContents.loadURL(origin);
    assert.equal(win.webContents.getTitle(), 'Restored natively');
    assert.equal(await sync.flush(), false);
    await mutation(jar, origin, () => win.webContents.executeJavaScript('localStorage.clear()'));
    assert.equal(await sync.flush(), false);
    assert.deepEqual(sent, []);
    online = true;
    assert.equal(await sync.flush(), true);
    assert.deepEqual(sent, [{ type: 'storage_changed', origins: { [origin]: {} } }]);
    await win.webContents.loadURL(origin);
    assert.equal(win.webContents.getTitle(), 'Signed out');
    sync.dispose();
    await win.webContents.executeJavaScript('localStorage.setItem("account", "Retired persona")');
    await assert.rejects(sync.flush(), /disposed/);
    assert.equal(sent.length, 1);
    console.log('PASS: native storage sync before navigation, offline logout retention, reconnect, and disposal');
  } finally {
    sync.dispose();
  }
};
