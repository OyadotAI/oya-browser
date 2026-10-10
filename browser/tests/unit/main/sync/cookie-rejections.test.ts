/** Native cookie diagnostics disclose fixed rejection categories, never saved login attributes. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { cookieRejectionCodes, cookieRejectionSummary } from '../../../../src/main/sync/cookie-rejections.ts';

test('counts only recognized native enum names without raw error text', () => {
  const reason = Error('secret=value private.example EXCLUDE_INVALID_PREFIX, EXCLUDE_USER_PREFERENCES');
  assert.deepEqual(cookieRejectionCodes(reason), ['EXCLUDE_INVALID_PREFIX', 'EXCLUDE_USER_PREFERENCES']);
});
test('unknown failures remain explicitly unclassified', () => {
  assert.deepEqual(cookieRejectionCodes(Error('storage unavailable secret=value')), ['unclassified']);
  assert.deepEqual(cookieRejectionCodes(Error('EXCLUDE_INVALID_PREFIX_EXTRA')), ['unclassified']);
});
test('aggregates each rejection and ignores successful writes', () => {
  assert.deepEqual(
    cookieRejectionSummary([
      { status: 'fulfilled', value: undefined },
      { status: 'rejected', reason: Error('EXCLUDE_INVALID_DOMAIN') },
      { status: 'rejected', reason: Error('EXCLUDE_INVALID_DOMAIN') },
      { status: 'rejected', reason: Error('unknown') },
    ]),
    { EXCLUDE_INVALID_DOMAIN: 2, unclassified: 1 },
  );
});
