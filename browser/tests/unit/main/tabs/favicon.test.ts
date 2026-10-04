/**
 * Unit tests for tab favicons: fetched through the tab's own session, only
 * images of icon size, a stale fetch dropped, and the icon cleared when the
 * tab leaves the site.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { TabManager } from '../../../../src/main/tabs/tabs.ts';
import { fetchIcon } from '../../../../src/main/tabs/favicon.ts';
import { mainCtx } from '../../support/main-ctx.cjs';
import { flush } from '../../support/fakes.cjs';

/** A session whose fetch answers from `responses` (url → { type, body, status }). */
function fakeSession(responses) {
  const fetched = [];
  return {
    fetched,
    fetch: async (url) => {
      fetched.push(url);
      const { type = 'image/png', body = 'PNG', status = 200 } = responses[url] || { status: 404 };
      return {
        ok: status < 400,
        headers: new Map([['content-type', type]]),
        arrayBuffer: async () => Buffer.from(body),
      };
    },
  };
}

describe('fetchIcon', () => {
  it('hands back an image as a data: URL', async () => {
    const session = fakeSession({ 'https://a.test/i.png': { type: 'image/png; q=1', body: 'PNG' } });
    assert.equal(await fetchIcon(session, 'https://a.test/i.png'), 'data:image/png;base64,UE5H');
  });

  it('refuses what is not an image, a failed answer, and anything too big to be an icon', async () => {
    const session = fakeSession({
      'https://a.test/page': { type: 'text/html' },
      'https://a.test/big': { body: 'x'.repeat(300_000) },
    });
    assert.equal(await fetchIcon(session, 'https://a.test/page'), null);
    assert.equal(await fetchIcon(session, 'https://a.test/missing'), null);
    assert.equal(await fetchIcon(session, 'https://a.test/big'), null);
  });
});

describe('a tab favicon', () => {
  let ctx, tab, session;
  /** What the strip was last told about the tab. */
  const shown = () => ctx.shell.sentOn('tabs-updated').at(-1)[0].favicon;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ tabs: TabManager });
    tab = ctx.tabs.find(ctx.tabs.createTab('https://a.test/'));
    session = fakeSession({ 'https://a.test/a.ico': { body: 'A' }, 'https://b.test/b.ico': { body: 'B' } });
    tab.view.webContents.session = session;
  });
  afterEach(() => mock.timers.reset());

  it("is fetched through the tab's own session, never the shell's", async () => {
    tab.view.webContents.emit('page-favicon-updated', {}, ['https://a.test/a.ico']);
    await flush();
    assert.deepEqual(session.fetched, ['https://a.test/a.ico']);
    assert.equal(shown(), 'data:image/png;base64,QQ==');
  });

  it('ignores addresses that are not web or data ones', async () => {
    tab.view.webContents.emit('page-favicon-updated', {}, ['file:///etc/icon.png']);
    await flush();
    assert.deepEqual(session.fetched, []);
  });

  it('drops a fetch the page overtook with another icon', async () => {
    tab.view.webContents.emit('page-favicon-updated', {}, ['https://a.test/a.ico']);
    tab.view.webContents.emit('page-favicon-updated', {}, ['https://b.test/b.ico']);
    await flush();
    assert.equal(shown(), 'data:image/png;base64,Qg==');
  });

  it('keeps the icon on the same site and clears it on another', async () => {
    tab.view.webContents.emit('did-navigate', {}, 'https://a.test/');
    tab.view.webContents.emit('page-favicon-updated', {}, ['https://a.test/a.ico']);
    await flush();
    tab.view.webContents.emit('did-navigate', {}, 'https://a.test/other');
    assert.equal(shown(), 'data:image/png;base64,QQ==');
    tab.view.webContents.emit('did-navigate', {}, 'https://b.test/');
    assert.equal(shown(), null);
  });
});
