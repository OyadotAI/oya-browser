/**
 * Unit tests for the operator's residential gateway: a sticky session per
 * persona and its exit country filled into the vendor URL, with the
 * credentials split out.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { residential } from '../../../../src/modules/proxies/residential.ts';

const TEMPLATE = 'http://user-country-{geo}-session-{session}:pa%40ss@gate.vendor.test:7000';

describe('residential', () => {
  it('is null when the operator has no gateway, or there is no persona', () => {
    assert.equal(residential({ id: 'p-1' }, {}), null);
    assert.equal(residential(null, { OYA_RESIDENTIAL_PROXY_URL: TEMPLATE }), null);
  });

  it('fills the geo and a sticky session into the gateway credentials', () => {
    const exit = residential({ id: 'p-1', proxy: { geo: 'DE-BE' } }, { OYA_RESIDENTIAL_PROXY_URL: TEMPLATE });
    assert.equal(exit.url, 'http://gate.vendor.test:7000');
    assert.match(exit.username, /^user-country-de-session-[0-9a-f]{16}$/);
    assert.equal(exit.password, 'pa@ss');
    assert.equal(exit.geo, 'DE');
  });

  it('keeps the same session for a persona, and a different one per persona', () => {
    const env = { OYA_RESIDENTIAL_PROXY_URL: TEMPLATE };
    assert.equal(residential({ id: 'p-1' }, env).username, residential({ id: 'p-1' }, env).username);
    assert.notEqual(residential({ id: 'p-1' }, env).username, residential({ id: 'p-2' }, env).username);
  });

  it("uses the operator's geo, else US, when the persona names none", () => {
    assert.equal(
      residential({ id: 'p-1' }, { OYA_RESIDENTIAL_PROXY_URL: TEMPLATE, OYA_RESIDENTIAL_PROXY_GEO: 'gb' }).geo,
      'GB',
    );
    assert.equal(residential({ id: 'p-1' }, { OYA_RESIDENTIAL_PROXY_URL: TEMPLATE }).geo, 'US');
  });

  it('gives null credentials for a gateway without them', () => {
    const exit = residential({ id: 'p-1' }, { OYA_RESIDENTIAL_PROXY_URL: 'http://gate.vendor.test:7000' });
    assert.deepEqual([exit.username, exit.password], [null, null]);
  });
});
