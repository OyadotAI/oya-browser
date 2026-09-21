/**
 * Unit tests for the credential re-check: every gateway client, browser and
 * stream viewer on this replica is re-authenticated; a revoked credential is
 * closed with 4003, an outage with 1013 so desktop browsers reconnect.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-access-');
const { control, projectId } = await import('../../../../../src/modules/control/service.ts');
const { validateAttachments } = await import('../../../../../src/modules/control/worker/access.ts');
const { registry } = await import('../../../../../src/modules/browsers/registry.ts');
const { sessions: gateways } = await import('../../../../../src/modules/gateway/service.ts');
const { connectBrowser, disconnectBrowser } = await import('../../../support/fakes.ts');
const { patchRow } = await import('../../../support/control.ts');

let good, revoked;
beforeEach(async () => {
  good = (await control().credential('key-a')).token;
  const bad = await control().credential('key-a');
  await control().revoke('key-a', bad.id);
  revoked = bad.token;
});
afterEach(() => {
  gateways.clear();
  disconnectBrowser('b1');
  mock.restoreAll();
});

/** Connects browser b1 authenticated with `token`, returning its socket. */
function browser(token) {
  const ws = connectBrowser('b1');
  registry.get('b1').authToken = token;
  return ws;
}
/** A stream viewer authenticated with `token`. */
const viewer = (token) => ({ authToken: token, end: mock.fn() });

describe('validateAttachments', () => {
  it('leaves connections whose credential still passes', async () => {
    const ws = browser(good);
    const client = { close: mock.fn() };
    gateways.set('g1', { authToken: good, client });
    await validateAttachments();
    assert.equal(ws.closed, null);
    assert.equal(client.close.mock.callCount(), 0);
  });

  it('closes a gateway client whose credential was revoked with a policy close', async () => {
    const client = { close: mock.fn() };
    gateways.set('g1', { authToken: revoked, client });
    await validateAttachments();
    assert.deepEqual(client.close.mock.calls[0].arguments, [1008, 'Credential revoked or validation unavailable']);
  });

  it('closes a revoked browser with 4003 and ends its viewers', async () => {
    const ws = browser(revoked);
    const v = viewer(good);
    registry.get('b1').streamViewers.add(v);
    await validateAttachments();
    assert.equal(ws.closed.code, 4003);
    assert.ok(v.end.mock.callCount() >= 1);
  });

  it('closes with 1013 when validation is unavailable, so the browser reconnects', async () => {
    const ws = browser(good);
    const { key } = await control().store.get('project', projectId('key-a'));
    await patchRow(control(), 'project', projectId('key-a'), { key: 'garbage' });
    try {
      await validateAttachments();
    } finally {
      await patchRow(control(), 'project', projectId('key-a'), { key });
    }
    assert.equal(ws.closed.code, 1013);
  });

  it('removes only the viewer whose own credential was revoked', async () => {
    const ws = browser(good);
    const kept = viewer(good),
      dropped = viewer(revoked);
    registry.get('b1').streamViewers.add(kept).add(dropped);
    await validateAttachments();
    assert.equal(ws.closed, null);
    assert.equal(kept.end.mock.callCount(), 0);
    assert.equal(dropped.end.mock.callCount(), 1);
    assert.equal(registry.get('b1').streamViewers.has(dropped), false);
  });

  it('checks each credential once per round', async () => {
    const authenticate = mock.method(control(), 'authenticate');
    gateways.set('g1', { authToken: good, client: { close() {} } });
    gateways.set('g2', { authToken: good, client: { close() {} } });
    await validateAttachments();
    assert.equal(authenticate.mock.callCount(), 1);
  });
});
