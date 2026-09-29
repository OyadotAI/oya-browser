/**
 * Unit tests for main/exit-zone.cjs: a proxied persona's exit timezone is asked
 * through its own session, the proxy's auth challenge is answered with the
 * persona's credentials, and anything but a real IANA zone is no answer.
 * Electron's net is faked.
 */
const { describe, it, before, after } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { installElectron, freshRequire } = require('../support/fakes.cjs');

const PROXY = { host: 'gate.test', port: 8080, username: 'user-session-1', password: 'pw' };
let next;

/** A net.request that challenges for proxy auth, then answers `next.body` (or fails with `next.error`). */
function fakeRequest(options) {
  const req = new EventEmitter();
  req.options = options;
  req.abort = () => {};
  req.end = () => setImmediate(() => play(req));
  next.requests.push(req);
  return req;
}

/** Plays the scripted exchange on `req`: the challenge, then the response or the error. */
function play(req) {
  req.emit(
    'login',
    { isProxy: true, host: PROXY.host },
    (username, password) => (req.credentials = [username, password]),
  );
  if (next.error) return req.emit('error', next.error);
  const res = new EventEmitter();
  req.emit('response', res);
  res.emit('data', Buffer.from(JSON.stringify(next.body)));
  res.emit('end');
}

describe('exitTimezone', () => {
  let restore;
  let exitTimezone;
  before(() => {
    restore = installElectron({ net: { request: fakeRequest } });
    ({ exitTimezone } = freshRequire('main/exit-zone.cjs'));
  });
  after(() => restore());

  it("answers the exit's zone, asked through the persona's session and answering its proxy's challenge", async () => {
    next = { requests: [], body: { ip: '203.0.113.9', timezone: 'America/Denver' } };
    const ses = {};
    assert.equal(await exitTimezone(ses, PROXY, 'https://geo.test/json'), 'America/Denver');
    const [req] = next.requests;
    assert.equal(req.options.session, ses);
    assert.equal(req.options.url, 'https://geo.test/json');
    assert.deepEqual(req.credentials, ['user-session-1', 'pw']);
  });

  it('gives no answer for a field that is not a timezone, or a lookup that fails', async () => {
    next = { requests: [], body: { timezone: 'Mars/Olympus' } };
    assert.equal(await exitTimezone({}, PROXY), null);
    next = { requests: [], body: { city: 'Denver' } };
    assert.equal(await exitTimezone({}, PROXY), null);
    next = { requests: [], error: new Error('net::ERR_TUNNEL_CONNECTION_FAILED') };
    assert.equal(await exitTimezone({}, PROXY), null);
  });
});
