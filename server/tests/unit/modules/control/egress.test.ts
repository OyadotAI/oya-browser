/**
 * Unit tests for managed-browser egress: the host rule matcher, and the proxy's
 * refusals, missing or wrong session credentials, ended sessions, non-standard
 * ports, hosts outside the policy, human-only hosts without a human in control,
 * and private destinations. Requests are driven into the server's handlers
 * directly; the relayed happy path needs a real upstream and is not covered.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-control-egress-');
const { allowedHost, createEgressServer } = await import('../../../../src/modules/control/egress.ts');
const { control, hash } = await import('../../../../src/modules/control/service.ts');
const { patchRow, readySession } = await import('../../support/control.ts');

describe('allowedHost', () => {
  it('matches an exact name, case- and trailing-dot-insensitively', () => {
    assert.equal(allowedHost('Example.COM.', ['example.com']), true);
    assert.equal(allowedHost('www.example.com', ['example.com']), false);
  });

  it('matches *.domain on any subdomain depth but never the bare domain', () => {
    assert.equal(allowedHost('a.b.example.com', ['*.example.com']), true);
    assert.equal(allowedHost('_dmarc.example.com', ['*.example.com']), true);
    assert.equal(allowedHost('example.com', ['*.example.com']), false);
    assert.equal(allowedHost('evilexample.com', ['*.example.com']), false);
  });

  it('lets a mid-label wildcard match within one label only', () => {
    assert.equal(allowedHost('us-aiplatform.googleapis.com', ['*-aiplatform.googleapis.com']), true);
    assert.equal(allowedHost('a.b-aiplatform.googleapis.com', ['*-aiplatform.googleapis.com']), false);
  });

  it('treats regex characters in a rule literally', () => {
    assert.equal(allowedHost('exampleXcom', ['example.com']), false);
  });

  it('refuses a host longer than DNS allows', () => {
    assert.equal(allowedHost(`${'a.'.repeat(130)}com`, ['*.com']), false);
  });
});

/** Proxy-Authorization for session `id` with `token`. */
const basic = (id, token) => `Basic ${Buffer.from(`${id}:${token}`).toString('base64')}`;

/** Drives one plain-HTTP proxy request; resolves with the status written. */
function proxyRequest(headers, url = 'http://example.com/') {
  const server = createEgressServer();
  return new Promise<number>((resolve) => {
    const req = { headers, url, method: 'GET', on() {}, pipe() {} };
    const res = { writeHead: (status) => resolve(status), end() {}, headersSent: false };
    server.emit('request', req, res);
  });
}

/** Drives one CONNECT; resolves with the raw answer written to the socket. */
function connect(headers, target = 'example.com:443') {
  const server = createEgressServer();
  return new Promise<string>((resolve) => {
    const socket = { end: (text) => resolve(text), on() {}, destroy() {} };
    server.emit('connect', { headers, url: target }, socket, Buffer.alloc(0));
  });
}

let n = 0,
  id;
const TOKEN = 'egress-token';
beforeEach(async () => {
  id = `e-${n++}`;
  await readySession(control(), 'key-a', id);
  await patchRow(control(), 'session', id, {
    managed: true,
    egressHash: hash(TOKEN),
    policies: [{ allowedHosts: ['example.com', '10.0.0.1', 'bank.example'], humanHosts: ['bank.example'] }],
  });
});

describe('egress proxy', () => {
  it('asks for credentials when none are given', async () => {
    assert.equal(await proxyRequest({}), 407);
    assert.match(await connect({}), /^HTTP\/1.1 407/);
  });

  it('refuses a wrong token, an unmanaged session, and an ended one', async () => {
    assert.equal(await proxyRequest({ 'proxy-authorization': basic(id, 'wrong') }), 403);
    assert.equal(await proxyRequest({ 'proxy-authorization': 'Bearer x' }), 403);
    await patchRow(control(), 'session', id, { state: 'cleanup_pending' });
    assert.equal(await proxyRequest({ 'proxy-authorization': basic(id, TOKEN) }), 403);
    await patchRow(control(), 'session', id, { state: 'ready', managed: false });
    assert.equal(await proxyRequest({ 'proxy-authorization': basic(id, TOKEN) }), 403);
  });

  it('refuses non-standard ports and HTTPS without CONNECT', async () => {
    const auth = { 'proxy-authorization': basic(id, TOKEN) };
    assert.equal(await proxyRequest(auth, 'http://example.com:8080/'), 403);
    assert.equal(await proxyRequest(auth, 'https://example.com/'), 403);
    assert.match(await connect(auth, 'example.com:8443'), /^HTTP\/1.1 403/);
  });

  it('refuses a host outside the session’s policy', async () => {
    assert.match(await connect({ 'proxy-authorization': basic(id, TOKEN) }, 'other.example:443'), /403 Forbidden/);
  });

  it('refuses a human-only host while the agent is in control', async () => {
    assert.match(await connect({ 'proxy-authorization': basic(id, TOKEN) }, 'bank.example:443'), /403 Forbidden/);
  });

  it('refuses a private destination even when the policy allows it', async () => {
    assert.equal(await proxyRequest({ 'proxy-authorization': basic(id, TOKEN) }, 'http://10.0.0.1/'), 403);
  });
});
