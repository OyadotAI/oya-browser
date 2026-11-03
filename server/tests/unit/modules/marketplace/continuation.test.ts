/** Encrypted setup contexts bind buyer identity, audience, product and a fixed expiry. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { sealText } from '../../../../src/platform/secrets.ts';
import { openContext, sealContext } from '../../../../src/modules/marketplace/continuation.ts';
import { CONTINUATION_MS, CONTINUATION_SCOPE } from '../../../../src/modules/marketplace/constants.ts';

process.env.OYA_PROFILE_SECRET = 'marketplace-hermetic-test-key-only';
const NOW = 1_800_000_000_000;
const BUYER = {
  accountId: '123456789012',
  licenseArn: 'arn:aws:license-manager::123456789012:license:l-test',
  productCode: 'product-oya',
};

describe('setup continuation', () => {
  it('authenticates identity without storing the original AWS token', () => {
    const cookie = sealContext(BUYER, NOW);
    const context = openContext(cookie, BUYER.productCode, NOW);
    assert.equal(context.accountId, BUYER.accountId);
    assert.equal(context.expiresAt, NOW + CONTINUATION_MS);
    assert.equal(context.RegistrationToken, undefined);
    assert.ok(!cookie.includes(BUYER.accountId));
  });

  it('expires exactly at the original deadline rather than extending on reads', () => {
    const cookie = sealContext(BUYER, NOW);
    openContext(cookie, BUYER.productCode, NOW + CONTINUATION_MS - 1);
    assert.throws(() => openContext(cookie, BUYER.productCode, NOW + CONTINUATION_MS), { code: 'setup_expired' });
  });

  it('rejects tampering and a changed expected product', () => {
    const cookie = sealContext(BUYER, NOW);
    const tampered = Buffer.from(cookie, 'base64');
    tampered[tampered.length - 1] ^= 1;
    assert.throws(() => openContext(tampered.toString('base64'), BUYER.productCode, NOW), { code: 'invalid_setup' });
    assert.throws(() => openContext(cookie, 'another-product', NOW), { code: 'invalid_setup' });
  });

  it('rejects another audience, malformed identities and unsupported versions even inside a valid envelope', () => {
    const original = openContext(sealContext(BUYER, NOW), BUYER.productCode, NOW);
    for (const change of [
      { audience: 'another-use' },
      { accountId: 'not-an-account' },
      { version: 99 },
      { nonce: '' },
      { expiresAt: 'tomorrow' },
    ]) {
      const cookie = sealText(CONTINUATION_SCOPE, { ...original, ...change });
      assert.throws(() => openContext(cookie, BUYER.productCode, NOW), { code: 'invalid_setup' });
    }
    assert.throws(() => openContext('', BUYER.productCode, NOW), { code: 'invalid_setup' });
  });
});
