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
import {
  AUTH_DEADLINE_MS,
  CloseCode,
  MS_PER_SECOND,
} from '../../../../../src/modules/browsers/connection/constants.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { control } from '../../../../../src/modules/control/service.ts';
import { container } from '../../../../../src/app/container.ts';
import { FakeSocket, disconnectBrowser } from '../../../support/fakes.ts';
import { stubControl } from '../../../support/browsers.ts';
import { stubFetch, json } from '../../../support/http.ts';
import * as keyConfig from '../../../../../src/modules/config/service.ts';
import { track } from '../../../../../src/modules/telemetry/service.ts';
import * as usage from '../../../../../src/platform/usage.ts';
import { openRelay } from '../../../../../src/modules/browsers/cdp-relay.ts';
import { WebSocket } from 'ws';

const B = 'b-conn';
/** A second browser on the same key. */
const OTHER = 'b-conn-other';
/** How long the first socket was connected before the reconnect. */
const MINUTE_MS = 60_000;
/** A wall-clock time to connect at (any non-zero one: usage reads a zero start as no clock). */
const CONNECTED_AT = Date.UTC(2026, 0, 1, 12);

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

  /** When set, close() starts the closing handshake and 'close' waits for hangUp(), as it does on a real ws. */
  lingering = false;

  /** Records the close and tells the connection, at once unless the socket lingers. */
  close(code: number, reason: string) {
    super.close(code, reason);
    if (!this.lingering) this.events.emit('close');
  }

  /** The closing handshake ends (or ws gives up on a peer that went quiet): 'close' fires now. */
  hangUp() {
    this.events.emit('close');
  }
}

/** Lets the connection's async handling run to completion. */
const settle = async () => {
  for (let i = 0; i < 20; i++) await new Promise((r) => setImmediate(r));
};

/** A connection on a fresh socket (or `ws`) that has sent `auth` (when given). */
async function connect(auth?: object, ws = new WireSocket()) {
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

  it('refuses a share link credential, even for its own shared browser', async () => {
    principal({ key: 'k-conn', role: 'operator', sessionId: B });
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

  // A real ws reports a server-side close only once the closing handshake ends, or after its
  // close timeout when the peer went quiet: the usual reason a browser reconnects at all.
  describe('when a reconnect replaces a socket whose close has not arrived yet', () => {
    /** Sockets left mid-close, hung up after each test so their heartbeats stop. */
    let lingering: WireSocket[] = [];
    /** A connection on a socket whose 'close' only fires on hangUp(), as on a real ws. */
    const connectLingering = (extra: object = {}) => {
      const ws = new WireSocket();
      ws.lingering = true;
      lingering.push(ws);
      return connect(auth(extra), ws);
    };
    /** Browsers this key is billed for right now. */
    const open = () => usage.current('k-conn').openBrowsers;

    beforeEach(() => usage.reset());
    afterEach(() => {
      for (const ws of lingering) ws.hangUp();
      lingering = [];
      registry.get(OTHER)?.ws?.close?.(1000, 'test over');
      usage.reset();
    });

    it('stops the browser being billed when the server then refuses the reconnect', async () => {
      const first = await connectLingering();
      mock.method(control(), 'adopt', async () => {
        throw new Error('capacity');
      });
      await connect(auth());
      first.hangUp();
      assert.equal(registry.isConnected(B), false);
      assert.equal(open(), 0);
    });

    it("books the replaced socket's time instead of restarting the clock over it", async () => {
      mock.timers.enable({ apis: ['Date'], now: CONNECTED_AT });
      await connectLingering();
      mock.timers.tick(MINUTE_MS);
      await connect(auth());
      assert.equal(usage.current('k-conn').browser_seconds, MINUTE_MS / MS_PER_SECOND);
      assert.equal(open(), 1);
    });

    it("frees the old persona's slot when the browser comes back as another persona", async () => {
      const [solo, other] = ['solo', 'other'].map((name) =>
        container.personas.create('k-conn', { name, maxConcurrent: 1 }),
      );
      await connectLingering({ persona: solo.id });
      await connect(auth({ persona: other.id }));
      assert.equal(container.personas.activeCount(solo.id), 0);
      await connect(auth({ browser_id: OTHER, persona: solo.id }));
      assert.equal(registry.isConnected(OTHER), true, 'another browser may now run as the freed persona');
    });

    it('ends the CDP relays the replaced socket carried', async () => {
      const first = await connectLingering();
      const opening = openRelay(registry.get(B), B);
      first.deliver({ type: 'cdp_opened', sid: first.ofType('cdp_open')[0].sid });
      const relay: any = await opening;
      await connect(auth());
      assert.equal(relay.readyState, WebSocket.CLOSED);
    });

    it('keeps a browser that reconnects as the same persona at its cap', async () => {
      const solo = container.personas.create('k-conn', { name: 'solo', maxConcurrent: 1 });
      await connectLingering({ persona: solo.id });
      const second = await connect(auth({ persona: solo.id }));
      assert.equal(registry.get(B).ws, second);
      assert.equal(container.personas.activeCount(solo.id), 1);
    });

    it("leaves the new connection's registration, slot and clock alone when the old close arrives late", async () => {
      const solo = container.personas.create('k-conn', { name: 'solo', maxConcurrent: 1 });
      const first = await connectLingering({ persona: solo.id });
      const second = await connect(auth({ persona: solo.id }));
      first.hangUp();
      assert.equal(registry.get(B).ws, second);
      assert.equal(container.personas.activeCount(solo.id), 1);
      assert.equal(open(), 1);
    });
  });
});
