/** Real browser-owned history traversal through Oya's external adapter, with no debugger access. */
const assert = require('node:assert/strict');
const { once } = require('node:events');
module.exports = async function historyChecks(a, b, wc) {
  const origin = new URL(wc.getURL()).origin;
  await wc.loadURL(`${origin}/history-first`);
  await wc.loadURL(`${origin}/history-second`);
  const history = await a.call('Oya.getNavigationHistory');
  const index = history.entries.findIndex((e) => e.url === `${origin}/history-first`);
  assert.ok(index >= 0);
  assert.equal(history.entries[history.currentIndex].url, `${origin}/history-second`);
  assert.ok(history.entries.every((e) => Object.keys(e).sort().join(',') === 'index,title,url'));
  await assert.rejects(b.call('Oya.navigateToHistoryEntry', { snapshot: history.snapshot, index }), /snapshot/);
  const loaded = once(wc, 'did-finish-load');
  await a.call('Oya.navigateToHistoryEntry', { snapshot: history.snapshot, index });
  await loaded;
  assert.equal(wc.getURL(), `${origin}/history-first`);
  await assert.rejects(a.call('Oya.navigateToHistoryEntry', { snapshot: history.snapshot, index }), /snapshot/);
  const back = await a.call('Oya.getNavigationHistory');
  const forward = once(wc, 'did-finish-load');
  await a.call('Oya.navigateToHistoryEntry', { snapshot: back.snapshot, index: history.currentIndex });
  await forward;
  assert.equal(wc.getURL(), `${origin}/history-second`);
  const stale = await a.call('Oya.getNavigationHistory');
  await wc.executeJavaScript('history.pushState({}, "", "#changed")');
  await assert.rejects(a.call('Oya.navigateToHistoryEntry', { snapshot: stale.snapshot, index }), /changed/);
  await wc.loadURL(`${origin}/blocked`);
  await wc.loadURL(`${origin}/history-second`);
  const denied = await a.call('Oya.getNavigationHistory');
  const blocked = denied.entries.findIndex((e) => e.url === `${origin}/blocked`);
  await assert.rejects(
    a.call('Oya.navigateToHistoryEntry', { snapshot: denied.snapshot, index: blocked }),
    /authorized/,
  );
  assert.equal(wc.getURL(), `${origin}/history-second`);
  console.log(
    'PASS: native tab history back/forward, private page-state exclusion, consumed/foreign/stale snapshots and egress-denied traversal',
  );
};
