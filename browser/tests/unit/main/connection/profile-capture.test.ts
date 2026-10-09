/** The internal stop handshake must never claim success for failed or foreign-persona captures. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { captureProfile } from '../../../../src/main/connection/profile-capture.ts';
/** A live authenticated socket and ordered capture functions. */
function fixture() {
  const order: any[] = [];
  const deps: any = {
    socket: { ready: true, ws: {}, send: (message) => order.push(message) },
    cookies: {
      captureProfile: async () => {
        order.push('cookies');
      },
    },
    persona: {
      active: { id: 'alice' },
      flushStorage: async () => {
        order.push('storage');
        return true;
      },
    },
  };
  return { deps, order };
}
it('answers only after native cookies and storage have reached the ordered transport', async () => {
  const { deps, order } = fixture();
  await captureProfile(deps, 'request');
  assert.deepEqual(order, ['cookies', 'storage', { type: 'cmd_result', id: 'request', ok: true }]);
});
it('reports capture failure without reporting native credential values', async () => {
  const { deps, order } = fixture();
  deps.cookies.captureProfile = async () => {
    throw Error('secret-token');
  };
  await captureProfile(deps, 'request');
  assert.equal(order[0].ok, false);
  assert.doesNotMatch(order[0].error, /secret-token/);
});
it('refuses success when native storage delivery is rejected', async () => {
  const { deps, order } = fixture();
  deps.persona.flushStorage = async () => false;
  await captureProfile(deps, 'request');
  assert.equal(order.at(-1).ok, false);
});
it('never acknowledges an old capture on a replacement connection', async () => {
  const { deps, order } = fixture();
  deps.cookies.captureProfile = async () => {
    deps.socket.ws = {};
  };
  await captureProfile(deps, 'request');
  assert.equal(
    order.some((entry) => entry?.type === 'cmd_result'),
    false,
  );
});
it('never acknowledges an old capture under a different persona', async () => {
  const { deps, order } = fixture();
  deps.persona.flushStorage = async () => {
    deps.persona.active.id = 'bob';
    return true;
  };
  await captureProfile(deps, 'request');
  assert.equal(
    order.some((entry) => entry?.type === 'cmd_result'),
    false,
  );
});
it('ignores uncorrelated requests and requests before authentication', async () => {
  const { deps, order } = fixture();
  await captureProfile(deps, undefined);
  deps.socket.ready = false;
  await captureProfile(deps, 'request');
  assert.deepEqual(order, []);
});
