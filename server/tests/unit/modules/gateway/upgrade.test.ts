/**
 * Unit tests for the /connect upgrade (upgrade.ts and its stages:
 * upgrade-auth, -context, -start, -acquire, -attach, -resume), driven through
 * a loopback HTTP server with a loopback browser behind the provider pool.
 * The control plane's lifecycle calls are recorded; its store is a scratch
 * SQLite file. Connect burst and browser quota are lowered for this file.
 */
import { describe, it, before, after, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { randomUUID } from 'node:crypto';
import type { AddressInfo } from 'node:net';
import { WebSocket } from 'ws';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

ownDataDir('oya-upgrade-');
process.env.OYA_LIMIT_CONNECT_BURST = '5';
process.env.OYA_QUOTA_MAX_BROWSERS = '1';
const { handleUpgrade, sessions } = await import('../../../../src/modules/gateway/service.ts');
const { pool } = await import('../../../../src/modules/gateway/routing.ts');
const profiles = await import('../../../../src/modules/gateway/profiles.ts');
const { registry } = await import('../../../../src/modules/browsers/registry.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { HttpError } = await import('../../../../src/platform/errors.ts');
const { recent, fingerprint } = await import('../../../../src/platform/audit.ts');
const { allowKey } = await import('../../support/agent.ts');
const { stubControl, fakeCdp, pageBrowser } = await import('../../support/gateway.ts');

/** A new key for each test, so the per-key connect budget and quota start fresh. */
let keyNo = 0;
const forgets: (() => void)[] = [];
const newKey = () => {
  const key = `upgrade-key-${++keyNo}`;
  forgets.push(allowKey(key));
  return key;
};

let port: number;
let server: ReturnType<typeof createServer>;
let browser: Awaited<ReturnType<typeof fakeCdp>>;
let control$: ReturnType<typeof stubControl>;
const clients: WebSocket[] = [];

/** Opens /connect with `query`; resolves with the socket once open, or the status it was refused with. */
function connect(query: string, headers: Record<string, string> = {}) {
  return new Promise<{ ws?: WebSocket; status?: number }>((resolve) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}/connect?${query}`, { headers });
    clients.push(ws);
    ws.once('open', () => resolve({ ws }));
    ws.once('unexpected-response', (_req, res) => resolve({ status: res.statusCode }));
    ws.once('error', () => resolve({ status: -1 }));
  });
}
/** Sends one CDP command and resolves with the reply. */
function command(ws: WebSocket, id: number, method: string) {
  return new Promise<any>((resolve) => {
    ws.on('message', (raw) => {
      const msg = JSON.parse(raw.toString());
      if (msg.id === id) resolve(msg);
    });
    ws.send(JSON.stringify({ id, method }));
  });
}
/** Waits until `check` holds. */
async function until(check: () => boolean) {
  for (let i = 0; i < 200 && !check(); i++) await new Promise((r) => setTimeout(r, 5));
  assert.ok(check(), 'condition never held');
}

before(async () => {
  browser = await fakeCdp(pageBrowser((method) => (method === 'Browser.getVersion' ? { product: 'Fake/1' } : {})));
  pool.register({ name: 'loopback', wsUrl: browser.url });
  server = createServer();
  server.on('upgrade', (req, socket, head) => handleUpgrade(req, socket, head));
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  port = (server.address() as AddressInfo).port;
});
after(async () => {
  forgets.forEach((f) => f());
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
  await browser.close();
});
beforeEach(() => {
  control$ = stubControl();
  mock.method(control() as any, 'reserve', async () => ({ id: randomUUID() }));
  mock.method(console, 'error', () => {});
});
afterEach(async () => {
  clients.splice(0).forEach((ws) => ws.terminate());
  await Promise.all([...sessions.values()].map((s) => s.destroy('test over')));
  mock.restoreAll();
  registry.draining = false;
});

describe('authentication', () => {
  it('refuses a connect without a key with a 401', async () => {
    assert.equal((await connect('')).status, 401);
  });

  it('refuses an unknown key with a 401', async () => {
    assert.equal((await connect('token=nobody')).status, 401);
  });

  it('accepts a key in the Authorization header', async () => {
    const key = newKey();
    const { ws } = await connect('', { Authorization: `Bearer ${key}` });
    assert.ok(ws);
  });

  it('refuses a key in the query when legacy query keys are turned off', async () => {
    const saved = process.env.OYA_ALLOW_LEGACY_QUERY_KEYS;
    process.env.OYA_ALLOW_LEGACY_QUERY_KEYS = 'false';
    assert.equal((await connect(`token=${newKey()}`)).status, 401);
    restoreEnv('OYA_ALLOW_LEGACY_QUERY_KEYS', saved);
  });

  it('redeems a connection ticket for the key it was issued to', async () => {
    const key = newKey();
    const redeem = mock.method(control() as any, 'redeem', async () => key);
    const { ws } = await connect('ticket=t-1');
    assert.ok(ws);
    assert.deepEqual(redeem.mock.calls[0].arguments, ['t-1', null]);
  });

  it('refuses a viewer credential with a 403', async () => {
    const key = newKey();
    mock.method(control() as any, 'authenticate', async () => ({ key, role: 'viewer' }));
    assert.equal((await connect('token=oya_viewer')).status, 403);
  });

  it('answers 503 when credentials cannot be checked', async () => {
    mock.method(control() as any, 'authenticate', async () => {
      throw new HttpError(503, 'store down');
    });
    assert.equal((await connect('token=oya_anything')).status, 503);
  });

  it('rate-limits connects per key with a 429', async () => {
    const key = newKey();
    for (let i = 0; i < 5; i++) assert.equal((await connect(`token=${key}&session=${randomUUID()}`)).status, 404);
    assert.equal((await connect(`token=${key}&session=${randomUUID()}`)).status, 429);
  });
});

describe('a new session', () => {
  it('relays CDP between the client and a browser from the pool', async () => {
    const key = newKey();
    const { ws } = await connect(`token=${key}`);
    const reply = await command(ws, 1, 'Browser.getVersion');
    assert.deepEqual(reply.result, { product: 'Fake/1' });
    const [s] = [...sessions.values()];
    assert.deepEqual([s.apiKey, s.provider, s.upstreamUrl], [key, 'loopback', browser.url]);
    assert.equal(control$.of('update')[0].args[2].state, 'ready');
    assert.equal(recent({ action: 'gateway.session.start' })[0].target_id, s.id);
  });

  it('hands the browser and its provider slot back when the session ends', async () => {
    const key = newKey();
    await connect(`token=${key}`);
    const [s] = [...sessions.values()];
    await s.destroy('done');
    assert.equal(pool.get(null, 'loopback').active, 0);
    assert.equal(control$.of('releaseProvider').length, 1);
  });

  it('refuses a second browser past the key’s quota with a 429', async () => {
    const key = newKey();
    await connect(`token=${key}`);
    assert.equal((await connect(`token=${key}`)).status, 429);
  });

  it('refuses while the server drains, with a 503', async () => {
    registry.draining = true;
    assert.equal((await connect(`token=${newKey()}`)).status, 503);
  });

  it('answers an admission refusal with its own status', async () => {
    mock.method(control() as any, 'reserve', async () => {
      throw new HttpError(402, 'Budget exhausted');
    });
    assert.equal((await connect(`token=${newKey()}`)).status, 402);
  });

  it('answers 503 when the provider pool has nothing, closing the reservation', async () => {
    mock.method(pool, 'acquire', async () => {
      throw new HttpError(503, 'No browser provider available');
    });
    assert.equal((await connect(`token=${newKey()}`)).status, 503);
    assert.equal(control$.of('complete')[0].args[2], 503);
  });

  it('answers 502 when every provider failed', async () => {
    mock.method(pool, 'acquire', async () => {
      throw new HttpError(502, 'All providers failed');
    });
    assert.equal((await connect(`token=${newKey()}`)).status, 502);
  });

  it('gives the browser back and answers 409 when the session was stopped while acquiring', async () => {
    mock.method(control() as any, 'update', async () => {
      throw new HttpError(409, 'stopped');
    });
    assert.equal((await connect(`token=${newKey()}`)).status, 409);
    assert.equal(pool.get(null, 'loopback').active, 0);
  });

  it('refuses a profile another session holds, with a 409', async () => {
    const key = newKey();
    profiles.tryLock(fingerprint(key), 'shop');
    assert.equal((await connect(`token=${key}&profile=shop`)).status, 409);
    profiles.unlock(fingerprint(key), 'shop');
  });

  it('holds the profile for the session and lets it go at the end', async () => {
    const key = newKey();
    await connect(`token=${key}&profile=shop`);
    assert.equal(profiles.isLocked(fingerprint(key), 'shop'), true);
    await [...sessions.values()][0].destroy('done');
    assert.equal(profiles.isLocked(fingerprint(key), 'shop'), false);
  });

  it('records the session when asked', async () => {
    const key = newKey();
    await connect(`token=${key}&record=1`);
    const [s] = [...sessions.values()];
    assert.equal(s.toJSON().recording, true);
  });

  it('refuses a session that cannot be recorded, ending it', async () => {
    const key = newKey();
    mock.method(control().store as any, 'get', async (kind) =>
      kind === 'session' ? { policies: [{ redactRecording: true }] } : null,
    );
    assert.equal((await connect(`token=${key}&record=1`)).status, 422);
    assert.equal(sessions.size, 0);
  });
});

describe('resuming a session', () => {
  it('gives a reconnecting client the same session', async () => {
    const key = newKey();
    const first = await connect(`token=${key}`);
    const [s] = [...sessions.values()];
    first.ws.close();
    await until(() => s.client === null);
    const again = await connect(`token=${key}&session=${s.id}`);
    assert.ok(again.ws);
    assert.equal((await command(again.ws, 2, 'Browser.getVersion')).result.product, 'Fake/1');
  });

  it("refuses another key's session with a 403", async () => {
    await connect(`token=${newKey()}`);
    const [s] = [...sessions.values()];
    s.client = null;
    assert.equal((await connect(`token=${newKey()}&session=${s.id}`)).status, 403);
  });

  it('refuses a session that already has a client with a 409', async () => {
    const key = newKey();
    await connect(`token=${key}`);
    const [s] = [...sessions.values()];
    assert.equal((await connect(`token=${key}&session=${s.id}`)).status, 409);
  });
});

describe('attaching to a fleet browser', () => {
  afterEach(() => ['b-att', 'b-oya', 'b-dead'].forEach((id) => registry.remove(id)));

  it('opens a second CDP client on a browser with an endpoint, recorded as an attachment', async () => {
    const key = newKey();
    registry.add('b-att', {
      apiKey: key,
      name: 'Fleet',
      clientType: 'cdp',
      provider: 'anchor',
      driver: { wsUrl: browser.url, close() {} },
    } as any);
    const { ws } = await connect(`token=${key}&browser=b-att`);
    assert.equal((await command(ws, 3, 'Browser.getVersion')).result.product, 'Fake/1');
    const [s] = [...sessions.values()];
    assert.deepEqual([s.attachedTo, s.provider, s.upstreamUrl], ['b-att', 'anchor', browser.url]);
    assert.equal((await control().store.get('attachment', s.id)).browserId, 'b-att');
  });

  it("refuses an unknown browser, or another key's, with a 404", async () => {
    const key = newKey();
    registry.add('b-att', {
      apiKey: 'someone-else',
      name: 'Fleet',
      clientType: 'cdp',
      driver: { wsUrl: browser.url, close() {} },
    } as any);
    assert.equal((await connect(`token=${key}&browser=b-att`)).status, 404);
    assert.equal((await connect(`token=${key}&browser=b-missing`)).status, 404);
  });

  it('refuses a browser with no CDP endpoint or relay, with a 409', async () => {
    const key = newKey();
    registry.add('b-oya', { apiKey: key, name: 'Client', clientType: 'oya' } as any);
    assert.equal((await connect(`token=${key}&browser=b-oya`)).status, 409);
  });

  it('answers 502 when the browser cannot be reached', async () => {
    const key = newKey();
    const dead = await fakeCdp();
    await dead.close();
    registry.add('b-dead', {
      apiKey: key,
      name: 'Gone',
      clientType: 'cdp',
      driver: { wsUrl: dead.url, close() {} },
    } as any);
    assert.equal((await connect(`token=${key}&browser=b-dead`)).status, 502);
  });
});
