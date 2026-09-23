/**
 * Unit tests for BrowserConnection, the life of one browser's control socket:
 * authenticating within the deadline, admission (bad keys, viewers, someone
 * else's browser id, draining, unknown personas), registration and the
 * welcome, reconnects that replace the old socket, and unregistering on close.
 * The control plane is stubbed on its singleton.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { BrowserConnection } from '../../../../../src/modules/browsers/connection/browser-connection.ts';
import { AUTH_DEADLINE_MS, CloseCode } from '../../../../../src/modules/browsers/connection/constants.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { control } from '../../../../../src/modules/control/service.ts';
import { container } from '../../../../../src/app/container.ts';
import { FakeSocket, disconnectBrowser } from '../../../support/fakes.ts';
import { stubControl } from '../../../support/browsers.ts';
import { stubFetch, json } from '../../../support/http.ts';
import * as keyConfig from '../../../../../src/modules/config/service.ts';
import { track } from '../../../../../src/modules/telemetry/service.ts';

const B = 'b-conn';

/** A FakeSocket that also carries the 'message' and 'close' events ws emits. */
class WireSocket extends FakeSocket {
  /** Where the connection's listeners live. */
  events = new EventEmitter();

  /** Subscribes, as ws.on does. */
  on(event: string, fn: (...args: any[]) => void) {
    this.events.on(event, fn);
  }

  /** Delivers one JSON message from the browser. */
  deliver(msg: object | string) {
    this.events.emit('message', Buffer.from(typeof msg === 'string' ? msg : JSON.stringify(msg)));
  }

  /** Records the close and tells the connection, as ws does. */
  close(code: number, reason: string) {
    super.close(code, reason);
    this.events.emit('close');
  }
}

/** Lets the connection's async handling run to completion. */
const settle = async () => {
  for (let i = 0; i < 20; i++) await new Promise((r) => setImmediate(r));
};

/** A connection on a fresh socket that has sent `auth` (when given). */
async function connect(auth?: object) {
  const ws = new WireSocket();
  new BrowserConnection(ws, { socket: { remoteAddress: '1.2.3.4' } });
  if (auth) ws.deliver({ type: 'auth', ...auth });
  await settle();
  return ws;
}

/** An auth message for B with the operator credential for `key`. */
const auth = (extra: object = {}) => ({ api_key: 'oya_op', browser_id: B, browser_name: 'Mine', ...extra });

/** Makes every credential the given principal. */
const principal = (p: object | null) => stubControl({ authenticate: async () => p });

describe('BrowserConnection', () => {
  beforeEach(() => {
    principal({ key: 'k-conn', role: 'operator' });
    mock.method(control(), 'adopt', async () => ({}));
    mock.method(control().store, 'get', async () => null);
    mock.method(control(), 'findSession', async () => ({ control: { mode: 'agent' } }));
    mock.method(console, 'log', () => {});
    mock.method(console, 'warn', () => {});
    mock.method(console, 'error', () => {});
  });
  afterEach(() => {
    const ws = registry.get(B)?.ws;
    ws?.close?.(1000, 'test over');
    disconnectBrowser(B);
    registry.draining = undefined;
    mock.restoreAll();
    mock.timers.reset();
  });

  it('registers an authenticated browser and welcomes it with its persona', async () => {
    const ws = await connect(auth());
    const [ok] = ws.ofType('auth_ok');
    assert.equal(ok.browser_id, B);
    assert.ok(ok.persona.id);
    const b = registry.get(B);
    assert.deepEqual([b.apiKey, b.name, b.clientType, b.ws], ['k-conn', 'Mine', 'oya', ws]);
  });

  it('tells the ops channel about a first desktop with its platform from a fixed list, never the text the client sent', async () => {
    process.env.SLACK_OPS_WEBHOOK_SIGNUPS = 'https://hooks.example.test/signups';
    keyConfig.reset();
    const calls = stubFetch(() => json({}));
    try {
      await connect(auth({ host_platform: '<!channel> <https://evil.example|Reset your password>' }));
      await new Promise((r) => setTimeout(r, 5));
      const lines = calls.map((c) => JSON.parse(c.init.body).text);
      assert.equal(lines.length, 1);
      assert.match(lines[0], /^🖥️ Desktop connected: .* \(unknown\)$/);
      assert.ok(!lines[0].includes('evil'));
    } finally {
      delete process.env.SLACK_OPS_WEBHOOK_SIGNUPS;
    }
  });

  it('records the desktop version it connects with, and a new one as an update', async () => {
    keyConfig.reset();
    const connected = mock.method(track, 'desktopConnected', () => {});
    const updated = mock.method(track, 'desktopUpdated', () => {});
    await connect(auth({ host_platform: 'MacIntel', app_version: '1.0.114' }));
    await connect(auth({ host_platform: 'MacIntel', app_version: '1.0.115' }));
    assert.deepEqual(
      connected.mock.calls.map((c) => c.arguments[1]),
      [
        { platform: 'MacIntel', first: true, version: '1.0.114' },
        { platform: 'MacIntel', first: false, version: '1.0.115' },
      ],
    );
    assert.deepEqual(
      updated.mock.calls.map((c) => c.arguments[1]),
      [{ platform: 'MacIntel', from: '1.0.114', to: '1.0.115' }],
    );
  });

  it('calls no update for an app too old to say its version', async () => {
    keyConfig.reset();
    const updated = mock.method(track, 'desktopUpdated', () => {});
    await connect(auth({ app_version: '1.0.114' }));
    await connect(auth());
    assert.equal(updated.mock.callCount(), 0);
  });

  it('keeps the actions a browser announced, sorted, and lists them on its detail', async () => {
    await connect(auth({ actions: ['wait', 'click', 'workflow', 'evaluate_raw', 'record'] }));
    // The internal names are dropped: callers are refused them, so the detail must not list them.
    assert.deepEqual(registry.describe(B).actions, ['click', 'wait', 'workflow']);
  });

  it('uses the Oya list for an app that announces none, or a list that is not plain action names', async () => {
    const tooMany = Array.from({ length: 129 }, (_, i) => `a_${'x'.repeat(i % 30)}`);
    for (const actions of [undefined, 'click', [1, 2], ['Click'], ['a'.repeat(41)], tooMany]) {
      await connect(auth(actions === undefined ? {} : { actions }));
      const listed = registry.describe(B).actions;
      assert.ok(listed.includes('workflow') && listed.includes('navigate'), JSON.stringify(actions)?.slice(0, 40));
      registry.get(B)?.ws?.close?.(1000, 'next');
      disconnectBrowser(B);
    }
    const warned = (console.warn as any).mock.calls.filter((c) => /unusable action list/.test(c.arguments[0]));
    assert.equal(warned.length, 5, 'one warning per unusable list, none for an app that sent nothing');
  });

  it('answers messages once authenticated', async () => {
    const ws = await connect(auth());
    ws.deliver({ type: 'ping' });
    await settle();
    assert.equal(ws.ofType('pong').length, 1);
  });

  it('ignores messages before authentication, and anything that is not a typed JSON message', async () => {
    const ws = await connect();
    ws.deliver({ type: 'ping' });
    ws.deliver('not json');
    ws.deliver({ no: 'type' });
    await settle();
    assert.equal(ws.sent.length, 0);
    assert.equal(ws.closed, null);
  });

  it('closes a socket that does not authenticate in time', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const ws = await connect();
    mock.timers.tick(AUTH_DEADLINE_MS);
    assert.deepEqual(ws.closed, { code: CloseCode.AUTH_TIMEOUT, reason: 'Auth timeout' });
  });

  it('refuses a second auth on the same socket', async () => {
    const ws = await connect(auth());
    ws.deliver({ type: 'auth', ...auth() });
    await settle();
    assert.deepEqual(ws.closed, { code: CloseCode.REJECTED, reason: 'Already authenticating' });
  });

  it('refuses an unknown key without registering the browser', async () => {
    principal(null);
    const ws = await connect(auth());
    assert.deepEqual(ws.closed, { code: CloseCode.REJECTED, reason: 'Invalid API key' });
    assert.equal(registry.isConnected(B), false);
  });

  it('refuses a viewer credential', async () => {
    principal({ key: 'k-conn', role: 'viewer' });
    assert.equal((await connect(auth())).closed.reason, 'Invalid API key');
  });

  it('refuses a managed browser credential for another session', async () => {
    principal({ key: 'k-conn', role: 'browser', sessionId: 'someone-else' });
    assert.equal((await connect(auth())).closed.reason, 'Invalid API key');
  });

  it('refuses a browser id registered to a different key', async () => {
    registry.add(B, { ws: new FakeSocket(), apiKey: 'k-other', name: 'Theirs' });
    const ws = await connect(auth());
    assert.deepEqual(ws.closed, { code: CloseCode.REJECTED, reason: 'browser_id registered to a different key' });
    assert.equal(registry.get(B).apiKey, 'k-other');
  });

  it('turns new browsers away while draining', async () => {
    registry.draining = true;
    assert.equal((await connect(auth())).closed.code, CloseCode.DRAINING);
  });

  it('runs an unknown or unowned persona as the key default instead of locking the browser out', async () => {
    const ws = await connect(auth({ persona: 'p-not-mine' }));
    const [ok] = ws.ofType('auth_ok');
    assert.equal(ok.persona.id, container.personas.defaultFor('k-conn').id);
    assert.notEqual(ok.persona.id, 'p-not-mine');
  });

  it('closes the socket and frees the persona when the control plane refuses the session', async () => {
    mock.method(control(), 'adopt', async () => {
      throw new Error('capacity');
    });
    const released = mock.method(container.personas, 'release');
    const ws = await connect(auth());
    assert.deepEqual(ws.closed, { code: CloseCode.CONTROL_REJECTED, reason: 'Control plane rejected connection' });
    assert.equal(released.mock.callCount(), 1);
    assert.equal(registry.isConnected(B), false);
  });

  it('replaces the old socket when the same browser reconnects on the same key', async () => {
    const first = await connect(auth());
    const second = await connect(auth());
    assert.deepEqual(first.closed, { code: CloseCode.REPLACED, reason: 'Replaced by new connection' });
    assert.equal(registry.get(B).ws, second);
  });

  it('unregisters the browser and frees its persona when its socket closes', async () => {
    const ws = await connect(auth());
    const released = mock.method(container.personas, 'release');
    ws.close(1000, 'bye');
    assert.equal(registry.isConnected(B), false);
    assert.equal(released.mock.callCount(), 1);
  });
});
