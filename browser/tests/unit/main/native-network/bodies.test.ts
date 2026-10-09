/** Original response retention is bounded, revocable and never re-fetches a URL. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeBodies } from '../../../../src/main/native-network/bodies.ts';
test('native bytes remain binary base64 and completion cannot replace an earlier result', async () => {
  const bodies = new NativeBodies();
  bodies.add(1);
  bodies.finish({ id: 1, body: 'AP8=', error: '' });
  bodies.finish({ id: 1, body: 'other', error: '' });
  assert.deepEqual(await bodies.read(1), { body: 'AP8=', base64Encoded: true });
  bodies.clear();
  await assert.rejects(bodies.read(1), /unavailable/);
});
test('revocation rejects readers already waiting on native stream completion', async () => {
  const bodies = new NativeBodies();
  bodies.add(1);
  const pending = assert.rejects(bodies.read(1), /revoked/);
  bodies.clear();
  await pending;
});
test('native overflow is an explicit failure and is never a truncated successful response', async () => {
  const bodies = new NativeBodies();
  bodies.add(1);
  bodies.finish({ id: 1, body: '', error: 'Native response body exceeds capture limit' });
  await assert.rejects(bodies.read(1), /limit/);
  bodies.clear();
});
test('body wait deadline settles without retrying or removing the still-pending original stream', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const bodies = new NativeBodies();
  bodies.add(1);
  const pending = assert.rejects(bodies.read(1), /still pending/);
  t.mock.timers.tick(3000);
  await pending;
  bodies.finish({ id: 1, body: 'ZmluYWw=', error: '' });
  assert.deepEqual(await bodies.read(1), { body: 'ZmluYWw=', base64Encoded: true });
  bodies.clear();
});
