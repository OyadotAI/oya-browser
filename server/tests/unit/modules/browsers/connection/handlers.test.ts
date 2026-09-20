/**
 * Unit tests for the browser message handlers, driven through a fake
 * connection: liveness, cookies and desktop control.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { HANDLERS, type Connection } from '../../../../../src/modules/browsers/connection/handlers/index.ts';
import { MAX_CONTROL_ID } from '../../../../../src/modules/browsers/connection/constants.ts';
import { FakeSocket } from '../../../support/fakes.ts';

/** A connection whose socket records what handlers send. */
function fakeConnection(overrides: Partial<Connection> = {}) {
  const ws = new FakeSocket();
  const conn: Connection = {
    browserId: 'b-handlers',
    apiKey: 'key-a',
    persona: { id: 'p-handlers' },
    residentialProxy: false,
    localCommands: new Map(),
    changingControl: false,
    heard: mock.fn(),
    send: (message) => ws.send(JSON.stringify(message)),
    isOpen: () => true,
    isCurrent: () => true,
    ...overrides,
  };
  return { conn, ws };
}

describe('liveness handlers', () => {
  it('answers a ping with a pong and counts it as a sign of life', () => {
    const { conn, ws } = fakeConnection();
    HANDLERS.ping(conn, { type: 'ping' });
    assert.equal(ws.ofType('pong').length, 1);
    assert.equal((conn.heard as any).mock.callCount(), 1);
  });

  it('ignores an unknown message type', () => {
    assert.equal(HANDLERS['not-a-type'], undefined);
  });
});

describe('cookie handlers', () => {
  it('answers a cookie pull with the jar for those hosts, echoing the pull id', () => {
    const { conn, ws } = fakeConnection();
    HANDLERS.cookie_pull(conn, { type: 'cookie_pull', domains: ['example.com'], pullId: 'p1' });
    const [sync] = ws.ofType('cookie_sync');
    assert.equal(sync.pullId, 'p1');
    assert.ok(Array.isArray(sync.cookies));
  });
});

describe('desktop control handler', () => {
  it('refuses a request id longer than the limit without answering', async () => {
    const { conn, ws } = fakeConnection();
    await HANDLERS.desktop_control(conn, { id: 'x'.repeat(MAX_CONTROL_ID + 1), action: 'take' });
    assert.equal(ws.sent.length, 0);
  });

  it('ignores messages from a socket that is no longer the browser’s', async () => {
    const { conn, ws } = fakeConnection({ isCurrent: () => false });
    await HANDLERS.desktop_control(conn, { id: 'r1', action: 'take' });
    assert.equal(ws.sent.length, 0);
  });

  it('refuses a second handoff while one is in progress', async () => {
    const { conn, ws } = fakeConnection({ changingControl: true });
    await HANDLERS.desktop_control(conn, { id: 'r2', action: 'take' });
    assert.deepEqual(ws.ofType('desktop_control_result')[0], {
      type: 'desktop_control_result',
      id: 'r2',
      error: 'A control handoff is already in progress',
    });
  });
});
