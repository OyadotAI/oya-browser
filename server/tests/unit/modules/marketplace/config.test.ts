/** Marketplace registration is disabled by default and refuses unsafe deployment settings. */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { marketplaceConfig } from '../../../../src/modules/marketplace/config.ts';

const NAMES = [
  'OYA_AWS_MARKETPLACE_ENABLED',
  'OYA_AWS_MARKETPLACE_PRODUCT_CODE',
  'OYA_AWS_MARKETPLACE_REGION',
  'OYA_PROFILE_SECRET',
];
const saved = Object.fromEntries(NAMES.map((name) => [name, process.env[name]]));
beforeEach(() =>
  NAMES.forEach((name) => {
    delete process.env[name];
  }),
);
afterEach(() =>
  NAMES.forEach((name) => {
    if (saved[name] === undefined) delete process.env[name];
    else process.env[name] = saved[name];
  }),
);

describe('marketplace configuration', () => {
  it('requires explicit opt-in, product and shared encryption secret', () => {
    assert.throws(() => marketplaceConfig('https://oya.example'), { code: 'unavailable' });
    process.env.OYA_AWS_MARKETPLACE_ENABLED = 'true';
    process.env.OYA_AWS_MARKETPLACE_PRODUCT_CODE = ' product-oya ';
    assert.throws(() => marketplaceConfig('https://oya.example'), { code: 'unavailable' });
    process.env.OYA_PROFILE_SECRET = 'test-only-shared-secret';
    assert.deepEqual(marketplaceConfig('https://oya.example/'), {
      productCode: 'product-oya',
      region: 'us-east-1',
      origin: 'https://oya.example',
    });
  });

  it('permits loopback development but refuses public HTTP and credential-bearing URLs', () => {
    Object.assign(process.env, {
      OYA_AWS_MARKETPLACE_ENABLED: 'true',
      OYA_AWS_MARKETPLACE_PRODUCT_CODE: 'p',
      OYA_PROFILE_SECRET: 'test',
    });
    assert.equal(marketplaceConfig('http://localhost:3100').origin, 'http://localhost:3100');
    for (const url of ['http://oya.example', 'https://user:secret@oya.example', 'invalid'])
      assert.throws(() => marketplaceConfig(url), { code: 'unavailable' });
  });
});
