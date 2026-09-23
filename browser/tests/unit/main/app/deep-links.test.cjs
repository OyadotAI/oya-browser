/**
 * Unit tests for DeepLinks: links held until the window exists, a retarget
 * that drops the old browser id only when the project changes, and a
 * cancelled pairing that changes nothing.
 */
const { describe, it, beforeEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { DeepLinks } = require('../../../../main/app/deep-links.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

describe('DeepLinks', () => {
  let ctx, links;
  beforeEach(() => {
    ctx = mainCtx();
    ctx.config.values = { apiKey: 'k1', serverUrl: 'wss://a.test/ws' };
    links = new DeepLinks(ctx);
  });

  it("holds a macOS link until the window exists, then applies it with the command line's", async () => {
    ctx.shell.window = null;
    const event = { preventDefault: mock.fn() };
    links.onOpenUrl(event, 'oya://connect?code=1');
    assert.equal(event.preventDefault.mock.callCount(), 1);
    const applied = mock.method(links, 'applyDeepLink', async () => true);
    await links.drain(['/app', 'oya://connect?code=2', '--flag']);
    assert.deepEqual(
      applied.mock.calls.map((c) => c.arguments[0]),
      ['oya://connect?code=1', 'oya://connect?code=2'],
    );
  });

  it("applies a second launch's link and brings the window forward", () => {
    const applied = mock.method(links, 'applyDeepLink', async () => true);
    links.onSecondInstance(['/app', 'oya://connect?code=3']);
    assert.equal(applied.mock.calls[0].arguments[0], 'oya://connect?code=3');
    assert.equal(ctx.shell.window.shown, true);
  });

  it('keeps the browser id when the project is unchanged, and drops it otherwise', () => {
    links.retarget({ apiKey: 'k1', serverUrl: 'wss://a.test/ws', persona: 'p' });
    assert.equal(ctx.socket.browserId, 'b1');
    links.retarget({ apiKey: 'k2', serverUrl: 'wss://a.test/ws', persona: 'p' });
    assert.equal(ctx.socket.browserId, null);
    assert.deepEqual(ctx.config.values, {
      apiKey: 'k2',
      serverUrl: 'wss://a.test/ws',
      persona: 'p',
      signedOut: false,
      keyFromApp: true,
    });
    assert.equal(ctx.config.saves, 2);
  });

  it('changes nothing when the person cancels the pairing', async () => {
    assert.equal(await links.applyDeepLink('oya://connect?code=c&server=wss://b.test/ws'), false);
    assert.equal(ctx.config.values.apiKey, 'k1');
    assert.equal(ctx.socket.connects, undefined);
  });

  it('pairs, reconnects and shows the window once the person confirms', async () => {
    ctx.electron.dialog.answers.messageBox.push({ response: 1 });
    mock.method(globalThis, 'fetch', async () => ({ ok: true, json: async () => ({ apiKey: 'k9', persona: 'work' }) }));
    assert.equal(await links.applyDeepLink('oya://connect?code=c&server=wss://b.test/ws'), true);
    assert.deepEqual([ctx.config.values.apiKey, ctx.config.values.persona], ['k9', 'work']);
    assert.equal(ctx.socket.connects, 1);
    globalThis.fetch.mock.restore();
  });
});
