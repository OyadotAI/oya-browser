/**
 * Unit tests for DeepLinks: links held until the window exists, a retarget
 * that drops the old browser id only when the project changes, and a
 * cancelled pairing that changes nothing.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { DeepLinks } from '../../../../src/main/app/deep-links.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

describe('DeepLinks', () => {
  let ctx, links;
  beforeEach(() => {
    ctx = mainCtx();
    ctx.tabs = { createTab() {}, enterBrowsingMode() {} };
    ctx.config.values = { apiKey: 'k1', serverUrl: 'wss://a.test/ws' };
    links = new DeepLinks(ctx);
  });

  it('opens external HTTP links in new tabs without retargeting the account', async () => {
    ctx.shell.browsingMode = true;
    const opened = [];
    ctx.tabs.createTab = (url) => opened.push(url);
    assert.equal(await links.applyDeepLink('https://example.test/path'), true);
    assert.deepEqual(opened, ['https://example.test/path']);
    assert.ok(!ctx.config.saves);
  });

  it('enters browsing for an external link on a fresh launch', async () => {
    ctx.shell.browsingMode = false;
    const entered = mock.method(ctx.tabs, 'enterBrowsingMode');
    assert.equal(await links.applyDeepLink('http://example.test'), true);
    assert.equal(entered.mock.calls[0].arguments[0], 'http://example.test/');
  });

  it('queues web links before readiness, including Windows second launches', async () => {
    ctx.shell.window = null;
    links.onSecondInstance(['app', 'https://first.test', '--flag']);
    const applied = mock.method(links, 'applyDeepLink', async () => true);
    await links.drain(['app', 'http://second.test', 'file:///private']);
    assert.deepEqual(
      applied.mock.calls.map((call) => call.arguments[0]),
      ['https://first.test', 'http://second.test'],
    );
  });

  it('does not open an OS link while an agent controls the browser', async () => {
    ctx.control.state.interactive = false;
    const opened = mock.fn();
    ctx.tabs.createTab = opened;
    assert.equal(await links.applyDeepLink('https://example.test'), false);
    assert.equal(opened.mock.callCount(), 0);
    assert.equal(await links.applyDeepLink('https://user:secret@example.test'), false);
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
    assert.equal(ctx.mirror.importOnConnect, undefined, 'no import unless ticked');
    globalThis.fetch.mock.restore();
  });

  it('imports the logins once connected when the person left the box ticked', async () => {
    ctx.electron.dialog.answers.messageBox.push({ response: 1, checkboxChecked: true });
    mock.method(globalThis, 'fetch', async () => ({ ok: true, json: async () => ({ apiKey: 'k9' }) }));
    await links.applyDeepLink('oya://connect?code=c&server=wss://b.test/ws');
    assert.equal(ctx.mirror.importOnConnect, true);
    globalThis.fetch.mock.restore();
  });
});
