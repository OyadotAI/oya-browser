/** Only explicit network failure names are admitted, never native codes or coerced strings. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validateParams } from '../../../../src/main/native-front-door/validation.ts';
import { validateFailureReason, requireFailureEngine } from '../../../../src/main/native-network/failures.ts';
test('supported network failure names validate through the external protocol boundary', () => {
  for (const errorReason of [
    'Failed',
    'Aborted',
    'TimedOut',
    'AccessDenied',
    'ConnectionClosed',
    'ConnectionReset',
    'ConnectionRefused',
    'ConnectionAborted',
    'ConnectionFailed',
    'NameNotResolved',
    'InternetDisconnected',
    'AddressUnreachable',
    'BlockedByClient',
    'BlockedByResponse',
  ]) {
    validateFailureReason(errorReason);
    validateParams({ id: 1, method: 'Fetch.failRequest', params: { requestId: 'owned', errorReason } });
  }
  for (const errorReason of [undefined, null, '', -7, 'ERR_TIMED_OUT', 'timedout', {}])
    assert.throws(() => validateFailureReason(errorReason), /reason/);
});
test('default native cancellation stays compatible without claiming old-engine error selection', () => {
  const session = { webRequest: {} } as any;
  requireFailureEngine(session, false);
  requireFailureEngine(session, true, 'BlockedByClient');
  assert.throws(() => requireFailureEngine(session, true, 'Aborted'), /engine/);
});
test('network protocol validation rejects missing IDs, authentication interception and response-stage rules', () => {
  for (const method of ['Network.getResponseBody', 'Fetch.continueRequest', 'Fetch.failRequest'])
    assert.throws(() => validateParams({ id: 1, method, params: {} }), /requestId/);
  validateParams({ id: 1, method: 'Fetch.enable', params: { handleAuthRequests: false, patterns: [] } });
  assert.throws(
    () => validateParams({ id: 1, method: 'Fetch.enable', params: { handleAuthRequests: true } }),
    /authentication/,
  );
  assert.throws(
    () => validateParams({ id: 1, method: 'Fetch.enable', params: { patterns: [{ requestStage: 'Response' }] } }),
    /Request stage/,
  );
});
