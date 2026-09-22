/**
 * Unit tests for the one way a tab loads a page: it waits for protection to
 * settle, refuses a tab that could not be protected, and lets a tab with no
 * setup of its own (an adopted popup) load as before.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadInTab, whenProtected, isUnprotected } = require('../../../../main/tabs/load.cjs');
const { TAB_UNPROTECTED } = require('../../../../main/tabs/constants.cjs');

/** A tab whose view records what it loads. */
const tabOf = (extra = {}) => {
  const loaded = [];
  return { loaded, view: { webContents: { loadURL: async (url) => loaded.push(url) } }, ...extra };
};

describe('loadInTab', () => {
  it('refuses a tab that could not be protected and loads nothing', async () => {
    const tab = tabOf({ protection: 'failed', setup: Promise.resolve() });
    const err = await loadInTab(tab, 'https://a.test/').catch((e) => e);
    assert.deepEqual([isUnprotected(err), err.message, tab.loaded], [true, TAB_UNPROTECTED, []]);
  });

  it('waits for setup that is still pending before it loads', async () => {
    let settle;
    const tab = tabOf({ protection: 'pending', setup: new Promise((r) => (settle = r)) });
    const loading = loadInTab(tab, 'https://a.test/');
    await new Promise((r) => setImmediate(r));
    assert.deepEqual(tab.loaded, []);
    tab.protection = 'protected';
    settle();
    await loading;
    assert.deepEqual(tab.loaded, ['https://a.test/']);
  });

  it('loads in a tab with no setup of its own, as an adopted popup has none', async () => {
    const tab = tabOf();
    await loadInTab(tab, 'https://a.test/');
    assert.deepEqual(tab.loaded, ['https://a.test/']);
  });

  it('lets a missing tab through the protection wait', async () => {
    await whenProtected(undefined);
  });
});
