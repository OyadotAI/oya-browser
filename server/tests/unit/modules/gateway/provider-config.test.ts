/**
 * Unit tests for validateProviderConfig: a provider's name, type, CDP URL and
 * limits, refused with a 400 when bad.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { validateProviderConfig } from '../../../../src/modules/gateway/provider-config.ts';

describe('validateProviderConfig', () => {
  it('normalises a cdp provider, filling in defaults', () => {
    assert.deepEqual(validateProviderConfig({ name: ' chrome ', wsUrl: 'ws://h:9222/devtools' }), {
      name: 'chrome',
      owner: null,
      type: 'cdp',
      wsUrl: 'ws://h:9222/devtools',
      enabled: true,
      maxConcurrent: 10,
      priority: 100,
      weight: 1,
    });
  });

  it('needs no URL for a hosted vendor', () => {
    const cfg = validateProviderConfig({ name: 'bb', type: 'browserbase', owner: 'o', enabled: false });
    assert.deepEqual([cfg.type, cfg.wsUrl, cfg.owner, cfg.enabled], ['browserbase', null, 'o', false]);
  });

  it('refuses a missing or badly formed name', () => {
    for (const name of [undefined, '', '-lead', 'a'.repeat(81), 'no/slash']) {
      assert.throws(() => validateProviderConfig({ name, wsUrl: 'ws://h' }), {
        status: 400,
        message: /provider name of 1–80/,
      });
    }
    assert.throws(() => validateProviderConfig(null), { status: 400 });
  });

  it('refuses a malformed type', () => {
    assert.throws(() => validateProviderConfig({ name: 'x', type: 'Bad Type' }), {
      status: 400,
      message: 'Invalid provider type',
    });
  });

  it('refuses a cdp provider without a ws:// or wss:// URL', () => {
    for (const wsUrl of [undefined, 'http://h', 'nonsense']) {
      assert.throws(() => validateProviderConfig({ name: 'x', wsUrl }), {
        status: 400,
        message: /ws:\/\/ or wss:\/\//,
      });
    }
  });

  it('refuses limits that are not integers at or above their minimum', () => {
    const base = { name: 'x', wsUrl: 'ws://h' };
    assert.throws(() => validateProviderConfig({ ...base, maxConcurrent: 0 }), {
      message: 'maxConcurrent must be an integer of at least 1.',
    });
    assert.throws(() => validateProviderConfig({ ...base, priority: -1 }), {
      message: 'priority must be an integer of at least 0.',
    });
    assert.throws(() => validateProviderConfig({ ...base, weight: 1.5 }), {
      message: 'weight must be an integer of at least 1.',
    });
    assert.throws(() => validateProviderConfig({ ...base, weight: null }), { status: 400 });
    assert.throws(() => validateProviderConfig({ ...base, weight: '' }), { status: 400 });
  });

  it('accepts numeric strings for limits', () => {
    assert.equal(validateProviderConfig({ name: 'x', wsUrl: 'ws://h', priority: '0' }).priority, 0);
  });
});
