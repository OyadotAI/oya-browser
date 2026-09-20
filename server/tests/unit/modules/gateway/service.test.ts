/**
 * Unit tests for the gateway facade: listing a key's sessions and killing
 * one by id.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-gateway-service-');
const gateway = await import('../../../../src/modules/gateway/service.ts');
const { Session } = await import('../../../../src/modules/gateway/session.ts');
const { stubControl, FakeWs } = await import('../../support/gateway.ts');

/** Puts a session for `apiKey` in the store. */
function add(id: string, apiKey: string) {
  const s = new Session({
    id,
    apiKey,
    provider: 'chrome',
    release: async () => {},
    upstream: new FakeWs(),
    profile: null,
  });
  gateway.sessions.set(id, s);
  return s;
}

beforeEach(() => stubControl());
afterEach(() => {
  mock.restoreAll();
  gateway.sessions.clear();
});

describe('gateway service', () => {
  it("lists only the key's own sessions, or every one with all", () => {
    add('s-a', 'key-a');
    add('s-b', 'key-b');
    assert.deepEqual(
      gateway.listSessions('key-a').map((s) => s.id),
      ['s-a'],
    );
    assert.equal(gateway.listSessions('key-a', { all: true }).length, 2);
  });

  it('kills a session by id and says whether there was one', async () => {
    const s = add('s-a', 'key-a');
    assert.equal(await gateway.killSession('s-a'), true);
    assert.equal(s.closed, true);
    assert.equal(gateway.sessions.has('s-a'), false);
    assert.equal(await gateway.killSession('s-a'), false);
  });

  it('exports the discovery handlers, the upgrade handler and the client socket server', () => {
    assert.equal(typeof gateway.handleJsonVersion, 'function');
    assert.equal(typeof gateway.handleJsonList, 'function');
    assert.equal(typeof gateway.handleUpgrade, 'function');
    assert.ok(gateway.wss);
  });
});
