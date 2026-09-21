/**
 * Unit tests for the CDP URL handed to callers: always this server's gateway,
 * with a single-use ticket, over wss when the request came in over TLS.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { browserCdpUrl } from '../../../../../src/modules/browsers/lifecycle/cdp-url.ts';
import { DEFAULT_PORT } from '../../../../../src/modules/browsers/constants.ts';
import { fakeRequest, stubControl } from '../../../support/browsers.ts';

describe('browserCdpUrl', () => {
  afterEach(() => mock.restoreAll());

  it('points at this host’s /connect with a fresh ticket for the browser', async () => {
    const { ticket } = stubControl({ ticket: async () => 't/1' });
    const url = await browserCdpUrl(fakeRequest({ headers: { host: 'oya.test' } }), 'b 1');
    assert.equal(url, 'ws://oya.test/connect?ticket=t%2F1&browser=b%201');
    assert.deepEqual(ticket.mock.calls[0].arguments, ['key-a', 'b 1', 'key-a']);
  });

  it('uses wss behind a TLS-terminating proxy', async () => {
    stubControl();
    const req = fakeRequest({ headers: { host: 'oya.test', 'x-forwarded-proto': 'https' } });
    assert.match(await browserCdpUrl(req, 'b'), /^wss:\/\/oya\.test\//);
  });

  it('ties the ticket to the credential the caller used', async () => {
    const { ticket } = stubControl();
    await browserCdpUrl(fakeRequest({ headers: { host: 'h' }, extra: { authToken: 'oya_scoped' } }), 'b');
    assert.equal(ticket.mock.calls[0].arguments[2], 'oya_scoped');
  });

  it('names localhost and the default port when there is no Host header', async () => {
    stubControl();
    const saved = process.env.PORT;
    delete process.env.PORT;
    try {
      assert.match(await browserCdpUrl(fakeRequest(), 'b'), new RegExp(`^ws://localhost:${DEFAULT_PORT}/`));
    } finally {
      if (saved !== undefined) process.env.PORT = saved;
    }
  });
});
