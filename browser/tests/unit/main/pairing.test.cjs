/**
 * Unit tests for main/pairing.cjs: an oya:// link pairs only after the person
 * agrees, only over a safe transport, and only for a code the server redeems.
 * fetch is faked; nothing leaves the process.
 */
const { describe, it, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { pairFromLink } = require('../../../main/pairing.cjs');

/** An ask() double answering each dialog with `response`, recording what it was shown. */
function fakeAsk(response) {
  const asked = [];
  const ask = async (options) => {
    asked.push(options);
    return { response };
  };
  return { ask, asked };
}

/** Fakes fetch with a JSON answer; returns the recorded calls. */
function fakeClaim(status, body) {
  return mock.method(globalThis, 'fetch', async () => ({ ok: status < 400, status, json: async () => body }));
}

const LINK = 'oya://connect?code=abc&server=' + encodeURIComponent('wss://oya.example/ws');

describe('pairFromLink', () => {
  afterEach(() => mock.restoreAll());

  it('pairs after the person confirms, redeeming the code with the named server', async () => {
    const fetch = fakeClaim(200, { apiKey: 'key-1', persona: 'work' });
    const { ask, asked } = fakeAsk(1);
    assert.deepEqual(await pairFromLink(LINK, ask), {
      apiKey: 'key-1',
      persona: 'work',
      serverUrl: 'wss://oya.example/ws',
    });
    assert.equal(asked[0].message, 'Connect to oya.example?');
    assert.equal(asked[0].defaultId, 0, 'Cancel is the default');
    const [url, init] = fetch.mock.calls[0].arguments;
    assert.equal(url, 'https://oya.example/api/pairing/claim');
    assert.deepEqual([init.method, init.redirect, init.body], ['POST', 'error', '{"code":"abc"}']);
  });

  it('uses the default persona when the server names none', async () => {
    fakeClaim(200, { apiKey: 'k' });
    assert.equal((await pairFromLink(LINK, fakeAsk(1).ask)).persona, 'default');
  });

  it('does nothing when the person cancels', async () => {
    const fetch = fakeClaim(200, { apiKey: 'k' });
    assert.equal(await pairFromLink(LINK, fakeAsk(0).ask), false);
    assert.equal(fetch.mock.callCount(), 0);
  });

  it('refuses links that are not oya://, lack a code or server, or name a non-WebSocket server', async () => {
    const { ask, asked } = fakeAsk(1);
    for (const link of [
      'https://x.test/?code=a&server=wss://x',
      'oya://connect?server=wss://x.test',
      'oya://connect?code=a',
      'oya://connect?code=a&server=https://x.test',
      'oya://connect?code=a&server=nonsense',
      'not a url',
    ]) {
      assert.equal(await pairFromLink(link, ask), false, link);
    }
    assert.equal(asked.length, 0, 'the person is never asked about a bad link');
  });

  it('allows plaintext ws:// only to this machine', async () => {
    const plain = (host) => 'oya://connect?code=a&server=' + encodeURIComponent(`ws://${host}:3100/ws`);
    fakeClaim(200, { apiKey: 'k' });
    assert.equal(await pairFromLink(plain('evil.test'), fakeAsk(1).ask), false);
    const paired = await pairFromLink(plain('localhost'), fakeAsk(1).ask);
    assert.equal(paired.serverUrl, 'ws://localhost:3100/ws');
  });

  it('tells the person why when the server refuses the code', async () => {
    fakeClaim(410, { error: 'Code expired' });
    const { ask, asked } = fakeAsk(1);
    assert.equal(await pairFromLink(LINK, ask), false);
    assert.deepEqual(asked[1], {
      type: 'error',
      title: 'Could not pair',
      message: 'Pairing failed',
      detail: 'Code expired',
    });
  });

  it('reports the status when the server answers without a key', async () => {
    fakeClaim(500, {});
    const { ask, asked } = fakeAsk(1);
    await pairFromLink(LINK, ask);
    assert.equal(asked[1].detail, 'Pairing failed (500)');
  });
});
