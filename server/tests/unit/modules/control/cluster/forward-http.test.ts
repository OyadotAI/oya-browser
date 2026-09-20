/**
 * Unit tests for HTTP forwarding between replicas: requests for a target held
 * in another replica's memory are proxied to its owner with a signed hop and
 * the response streamed back; everything else is served locally.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { PassThrough } from 'node:stream';
import { ownDataDir, restoreEnv } from '../../../support/data-dir.ts';

ownDataDir('oya-control-forward-');
const { forwardHttp } = await import('../../../../../src/modules/control/cluster.ts');
const { hopHeader } = await import('../../../../../src/modules/control/cluster/hop.ts');
const { control } = await import('../../../../../src/modules/control/service.ts');
const { stubFetch, text } = await import('../../../support/http.ts');
const { patchRow, putRow, readySession } = await import('../../../support/control.ts');

/** An Express-like request. */
const request = (path, extra: any = {}) => ({
  baseUrl: '/api',
  path,
  url: path,
  originalUrl: `/api${path}`,
  method: 'GET',
  headers: {},
  query: {},
  ...extra,
});

/** An Express-like response that collects what is streamed into it. */
function response() {
  const res: any = new PassThrough();
  Object.assign(res, {
    statusCode: 200,
    headers: {},
    headersSent: false,
    body: '',
    status: (code) => ((res.statusCode = code), res),
    set: (name, value) => ((res.headers[name] = value), res),
    json: (value) => ((res.body = value), (res.headersSent = true), res),
    flushHeaders: () => (res.headersSent = true),
  });
  res.on('data', (chunk) => (res.body += chunk));
  res.done = new Promise((resolve) => res.on('finish', resolve));
  return res;
}

let saved, token;
beforeEach(async () => {
  saved = { url: process.env.OYA_INSTANCE_URL, secret: process.env.OYA_CLUSTER_SECRET };
  process.env.OYA_CLUSTER_SECRET = 'cluster-secret';
  delete process.env.OYA_INSTANCE_URL;
  token = (await control().credential('key-a')).token;
});
afterEach(() => {
  restoreEnv('OYA_INSTANCE_URL', saved.url);
  restoreEnv('OYA_CLUSTER_SECRET', saved.secret);
  mock.restoreAll();
});

/** Runs the middleware; resolves whether it passed the request on. */
async function forward(req, res = response()) {
  let passed = false;
  await forwardHttp(req, res, () => (passed = true));
  return passed;
}

describe('forwardHttp, served locally', () => {
  it('passes on routes that are not held in one replica’s memory', async () => {
    assert.equal(await forward(request('/personas/p1')), true);
  });

  it('passes on collection routes under owned prefixes', async () => {
    assert.equal(await forward(request('/browsers/start')), true);
  });

  it('serves everything when running alone, redeeming a ticket for the credential', async () => {
    await readySession(control(), 'key-a', 'solo');
    const ticket = await control().ticket('key-a', 'solo', token);
    const req: any = request('/browsers/solo', { query: { ticket } });
    assert.equal(await forward(req), true);
    assert.equal(req.authToken, token);
  });

  it('answers a forged hop with 403', async () => {
    const res = response();
    assert.equal(
      await forward(request('/browsers/b1', { headers: { 'x-oya-hop': `${Date.now()}.forged` } }), res),
      false,
    );
    assert.equal(res.statusCode, 403);
    assert.equal(res.body.code, 'invalid_hop');
  });
});

describe('forwardHttp, clustered', () => {
  let n = 0,
    id;
  beforeEach(async () => {
    process.env.OYA_INSTANCE_URL = 'https://replica-1';
    id = `remote-${n++}`;
    await readySession(control(), 'key-a', id);
    await patchRow(control(), 'session', id, { instance: 'replica-2', leaseUntil: Date.now() + 60_000 });
    await putRow(control(), 'instance', 'replica-2', {
      id: 'replica-2',
      url: 'https://replica-2:8443',
      leaseUntil: Date.now() + 60_000,
    });
  });

  it('proxies to the owner with the credential and a signed hop, streaming the answer back', async () => {
    const calls = stubFetch(() =>
      text('frame', 201, { 'content-type': 'text/plain', 'retry-after': '3', 'x-internal': 'no' }),
    );
    const res = response();
    const req = request(`/live/${id}`, {
      originalUrl: `/api/live/${id}?key=${token}&frame=1`,
      query: { key: token },
      headers: { 'mcp-session-id': 'm1' },
    });
    assert.equal(await forward(req, res), false);
    await res.done;
    assert.equal(calls[0].url, `https://replica-2:8443/api/live/${id}?frame=1`);
    assert.equal(calls[0].init.headers.Authorization, `Bearer ${token}`);
    assert.equal(calls[0].init.headers['mcp-session-id'], 'm1');
    assert.match(calls[0].init.headers['X-Oya-Hop'], /^\d+\.[0-9a-f]{64}$/);
    assert.equal(calls[0].init.body, undefined);
    assert.equal(res.statusCode, 201);
    assert.deepEqual(res.headers, { 'content-type': 'text/plain', 'retry-after': '3' });
    assert.equal(res.body, 'frame');
  });

  it('sends a JSON body for writes', async () => {
    const calls = stubFetch(() => text('', 200));
    const res = response();
    await forward(
      request(`/control/sessions/${id}/input`, {
        method: 'POST',
        body: { action: 'click' },
        headers: { authorization: `Bearer ${token}` },
      }),
      res,
    );
    await res.done;
    assert.equal(calls[0].init.body, '{"action":"click"}');
  });

  it('serves locally what another project asks about', async () => {
    const other = (await control().credential('key-b')).token;
    stubFetch(() => text('', 200));
    assert.equal(await forward(request(`/browsers/${id}`, { authToken: other })), true);
  });

  it('refuses to forward a request that was already forwarded once', async () => {
    const path = `/api/browsers/${id}`;
    const res = response();
    const req = request(`/browsers/${id}`, { authToken: token, headers: { 'x-oya-hop': hopHeader('GET', path) } });
    assert.equal(await forward(req, res), false);
    assert.equal(res.statusCode, 503);
    assert.equal(res.body.code, 'owner_changed');
  });

  it('answers 503 owner_unavailable when the owner cannot be reached', async () => {
    stubFetch(() => {
      throw new Error('ECONNREFUSED');
    });
    const res = response();
    await forward(request(`/browsers/${id}`, { authToken: token }), res);
    assert.equal(res.statusCode, 503);
    assert.equal(res.body.code, 'owner_unavailable');
  });
});
