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
    ctx.agentTab = { humanMay: () => true };
    ctx.config.values = { apiKey: 'k1', serverUrl: 'wss://a.test/ws' };
    links = new DeepLinks(ctx);
  });

  it('sends a connect-an-agent link from the OS to its dialog, never to pairing', async () => {
    const asked = [];
    links.ask = async (options) => (asked.push(options), { response: 0 });
    links.onOpenUrl({ preventDefault() {} }, 'oya://agent-connect?agent=not-an-agent');
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(asked[0].message, 'This link names an agent Oya does not know');
    assert.ok(!ctx.config.saves);
  });

  it('a connect-an-agent link opened by a page inside Oya asks nothing and changes nothing', async () => {
    const asked = [];
    links.ask = async (options) => (asked.push(options), { response: 1 });
    assert.equal(await links.applyDeepLink('oya://agent-connect?agent=claude-code'), false);
    assert.deepEqual(asked, []);
    assert.ok(!ctx.config.saves);
  });

  it('a connect-an-agent link that arrived before the window is answered once the window exists', async () => {
    const asked = [];
    links.ask = async (options) => (asked.push(options), { response: 0 });
    const window = ctx.shell.window;
    ctx.shell.window = null;
    links.onOpenUrl({ preventDefault() {} }, 'oya://agent-connect?agent=not-an-agent');
    assert.deepEqual(asked, []);
    ctx.shell.window = window;
    await links.drain([]);
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(asked.length, 1);
  });

  it('opens external HTTP links in new tabs without retargeting the account', async () => {
    ctx.shell.browsingMode = true;
    const opened = [];
    ctx.tabs.createTab = (url) => opened.push(url);
    assert.equal(await links.applyDeepLink('https://example.test/path'), true);
    assert.deepEqual(opened, ['https://example.test/path']);
    assert.ok(!ctx.config.saves);
  });

  it('opens an OS link while agents drive only their own tabs', async () => {
    ctx.shell.browsingMode = true;
    ctx.control.state.interactive = false;
    ctx.agentTab.humanMay = (id) => id === null;
    const opened = [];
    ctx.tabs.createTab = (url) => opened.push(url);
    assert.equal(await links.applyDeepLink('https://example.test/'), true);
    assert.deepEqual(opened, ['https://example.test/']);
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
    ctx.agentTab.humanMay = () => false;
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
      account: null,
      apiKey: 'k2',
      claimUrl: '',
      serverUrl: 'wss://a.test/ws',
      persona: 'p',
      signedOut: false,
      keyFromApp: true,
    });
    assert.equal(ctx.config.saves, 2);
  });

  it('forgets the remembered account when the key changes, and keeps it otherwise', () => {
    Object.assign(ctx.config.values, { account: { email: 'ada@example.com' }, claimUrl: 'https://a.test/claim/1' });
    links.retarget({ apiKey: ctx.config.values.apiKey, serverUrl: ctx.config.values.serverUrl, persona: 'p' });
    assert.deepEqual(ctx.config.values.account, { email: 'ada@example.com' });
    assert.equal(ctx.config.values.claimUrl, 'https://a.test/claim/1');
    links.retarget({ apiKey: 'other', serverUrl: ctx.config.values.serverUrl, persona: 'p' });
    assert.equal(ctx.config.values.account, null);
    assert.equal(ctx.config.values.claimUrl, '', 'the previous key had its own claim page');
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
