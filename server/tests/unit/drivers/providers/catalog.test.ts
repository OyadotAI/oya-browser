/**
 * Unit tests for the hosted-vendor catalog: presets merged with
 * OYA_BROWSER_PROVIDERS, reading dotted paths out of vendor responses, and
 * where keys and headers come from.
 */
import { describe, it, mock, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { catalog, firstPath, headersFor, keyFor } from '../../../../src/drivers/providers/catalog.ts';

afterEach(() => mock.restoreAll());

describe('firstPath', () => {
  it('returns the first path holding a non-empty string', () => {
    const body = { data: { cdp_url: '' }, cdp_url: 'wss://x', n: 5 };
    assert.equal(firstPath(body, ['data.cdp_url', 'n', 'missing.deep', 'cdp_url']), 'wss://x');
    assert.equal(firstPath(body, ['n']), null);
    assert.equal(firstPath(null, ['a']), null);
  });
});

describe('catalog', () => {
  it('lists the built-in vendors', () => {
    assert.deepEqual(Object.keys(catalog({})).sort(), ['anchor', 'browserbase', 'browseruse', 'steel']);
  });

  it('lets OYA_BROWSER_PROVIDERS override a preset’s fields and add a vendor', () => {
    const env = {
      OYA_BROWSER_PROVIDERS: JSON.stringify({ steel: { createUrl: 'https://proxy/steel' }, extra: { createUrl: 'x' } }),
    };
    const merged = catalog(env);
    assert.equal(merged.steel.createUrl, 'https://proxy/steel');
    assert.equal(merged.steel.wsQueryKey, 'apiKey', 'the rest of the preset stays');
    assert.equal(merged.extra.createUrl, 'x');
  });

  it('ignores overrides that are not JSON, logging why', () => {
    const error = mock.method(console, 'error', () => {});
    assert.deepEqual(Object.keys(catalog({ OYA_BROWSER_PROVIDERS: '{nope' })).length, 4);
    assert.equal(error.mock.callCount(), 1);
  });
});

describe('keys and headers', () => {
  it('reads a vendor’s key from <NAME>_API_KEY', () => {
    assert.equal(keyFor('steel', { STEEL_API_KEY: 'k' }), 'k');
    assert.equal(keyFor('steel', {}), '');
  });

  it('builds headers from a function or takes a fixed object', () => {
    assert.deepEqual(headersFor({ headers: (k: string) => ({ a: k }) }, 'key'), { a: 'key' });
    assert.deepEqual(headersFor({ headers: { b: '1' } }, 'key'), { b: '1' });
  });
});
