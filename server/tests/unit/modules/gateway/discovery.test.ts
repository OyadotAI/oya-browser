/**
 * Unit tests for Chrome's discovery endpoints as the gateway serves them:
 * /json/version points a CDP client at /connect, and /json/list shows a key
 * only its own sessions.
 */
import { describe, it, before, after, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-discovery-');
const { handleJsonVersion, handleJsonList } = await import('../../../../src/modules/gateway/discovery.ts');
const { sessions } = await import('../../../../src/modules/gateway/session-store.ts');
const { CHROME_VERSION } = await import('../../../../src/modules/gateway/constants.ts');
const { allowKey } = await import('../../support/agent.ts');

/** A response that records its status and JSON body. */
function response() {
  const res: any = { statusCode: 200, body: undefined };
  res.status = (s: number) => ((res.statusCode = s), res);
  res.json = (b: any) => ((res.body = b), res);
  return res;
}
/** A request for `url` with these headers. */
const request = (url: string, headers: Record<string, string> = {}) => ({ url, headers });

const KEY = 'discovery-key';
let forget: () => void;

describe('handleJsonVersion', () => {
  it('presents a real Chrome identity and the /connect socket on the host reached', () => {
    const res = response();
    handleJsonVersion(request('/json/version', { host: 'gw.example:8080' }), res);
    assert.deepEqual(res.body, { ...CHROME_VERSION, webSocketDebuggerUrl: 'ws://gw.example:8080/connect' });
  });

  it('uses wss behind an https proxy, and carries a token through', () => {
    const res = response();
    handleJsonVersion(request('/json/version?token=a b', { host: 'gw.example', 'x-forwarded-proto': 'https' }), res);
    assert.equal(res.body.webSocketDebuggerUrl, 'wss://gw.example/connect?token=a%20b');
  });

  it('falls back to localhost without a Host header', () => {
    const res = response();
    handleJsonVersion(request('/json/version'), res);
    assert.equal(res.body.webSocketDebuggerUrl, 'ws://localhost/connect');
  });
});

describe('handleJsonList', () => {
  before(() => (forget = allowKey(KEY)));
  after(() => forget());
  afterEach(() => {
    sessions.clear();
    mock.restoreAll();
  });

  it("lists the caller's sessions as pages it can reconnect to", async () => {
    sessions.set('s-1', { id: 's-1', apiKey: KEY, profile: 'shop' });
    sessions.set('s-2', { id: 's-2', apiKey: KEY, profile: null });
    sessions.set('s-3', { id: 's-3', apiKey: 'someone-else' });
    const res = response();
    await handleJsonList(request('/json/list', { host: 'gw', authorization: `Bearer ${KEY}` }), res);
    assert.deepEqual(res.body, [
      {
        id: 's-1',
        type: 'page',
        title: 'Gateway session (shop)',
        url: 'about:blank',
        webSocketDebuggerUrl: 'ws://gw/connect?session=s-1',
      },
      {
        id: 's-2',
        type: 'page',
        title: 'Gateway session',
        url: 'about:blank',
        webSocketDebuggerUrl: 'ws://gw/connect?session=s-2',
      },
    ]);
  });

  it('answers the authentication error for a missing or unknown key', async () => {
    const missing = response();
    await handleJsonList(request('/json/list'), missing);
    assert.deepEqual([missing.statusCode, missing.body], [401, { error: 'Missing API key' }]);
    const unknown = response();
    await handleJsonList(request('/json/list', { authorization: 'Bearer nope' }), unknown);
    assert.deepEqual([unknown.statusCode, unknown.body], [403, { error: 'Invalid API key' }]);
  });
});
