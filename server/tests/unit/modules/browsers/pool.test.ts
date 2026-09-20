/**
 * Unit tests for the browser pool: a key's browsers, round-robin dispatch
 * across them, and the pool's stats.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { getPoolBrowsers, nextBrowser, poolStats } from '../../../../src/modules/browsers/pool.ts';
import { registry } from '../../../../src/modules/browsers/registry.ts';
import { connectBrowser, disconnectBrowser } from '../../support/fakes.ts';

const IDS = ['p-1', 'p-2', 'p-other'];

describe('pool', () => {
  afterEach(() => IDS.forEach(disconnectBrowser));

  it('holds only the browsers of one key', () => {
    connectBrowser('p-1', 'pool-a');
    connectBrowser('p-other', 'pool-b');
    assert.deepEqual(getPoolBrowsers('pool-a'), ['p-1']);
  });

  it('hands out the key’s browsers in turn', () => {
    connectBrowser('p-1', 'pool-rr');
    connectBrowser('p-2', 'pool-rr');
    const picks = [nextBrowser('pool-rr'), nextBrowser('pool-rr'), nextBrowser('pool-rr')];
    assert.equal(picks[0], picks[2]);
    assert.notEqual(picks[0], picks[1]);
  });

  it('has no browser to hand out for an empty pool', () => {
    assert.equal(nextBrowser('pool-empty'), null);
  });

  it('reports its size with each browser’s name and page', () => {
    connectBrowser('p-1', 'pool-s');
    registry.updateUrl('p-1', 'https://example.com');
    assert.deepEqual(poolStats('pool-s'), {
      size: 1,
      browsers: [{ id: 'p-1', name: 'Test', currentUrl: 'https://example.com' }],
    });
  });
});
