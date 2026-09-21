/**
 * Unit tests for the browser and pool routers (routes.ts, pool-routes.ts):
 * every route sits behind authentication, and the routes that answer inline
 * (browser list, providers, pool status and cookies) answer for the caller's
 * key only. The handlers themselves are tested under http/.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { router as browsers } from '../../../../src/modules/browsers/routes.ts';
import { router as pool } from '../../../../src/modules/browsers/pool-routes.ts';
import { container } from '../../../../src/app/container.ts';
import { connectBrowser, disconnectBrowser } from '../../support/fakes.ts';
import { routeThrough, stubControl } from '../../support/browsers.ts';

const IDS = ['r-1', 'r-2'];

describe('browser and pool routes', () => {
  beforeEach(() => stubControl({ authenticate: async () => ({ key: 'k-routes', role: 'administrator' }) }));
  afterEach(() => {
    mock.restoreAll();
    IDS.forEach(disconnectBrowser);
  });

  it('refuses a request without a key', async () => {
    const res = await routeThrough(browsers, 'GET', '/browsers', '');
    assert.equal(res.statusCode, 401);
  });

  it('lists only the caller’s browsers', async () => {
    connectBrowser('r-1', 'k-routes');
    connectBrowser('r-2', 'k-other');
    const res = await routeThrough(browsers, 'GET', '/browsers');
    assert.deepEqual(
      res.body.map((b) => b.id),
      ['r-1'],
    );
  });

  it('names the CDP providers and whether Oya Cloud is available', async () => {
    const res = await routeThrough(browsers, 'GET', '/providers');
    assert.ok(Array.isArray(res.body.providers));
    assert.equal(typeof res.body.oyaCloud, 'boolean');
  });

  it('guards per-browser routes with the ownership check', async () => {
    connectBrowser('r-2', 'k-other');
    const res = await routeThrough(browsers, 'POST', '/browsers/r-2/command');
    assert.deepEqual([res.statusCode, res.body], [404, { error: 'Browser r-2 not connected' }]);
  });

  it('matches routes case-sensitively, so a guard cannot be dodged by casing', async () => {
    const res = await routeThrough(pool, 'GET', '/Pool/Cookies');
    assert.deepEqual(res.body, { unrouted: true });
  });

  it('shows the caller’s pool', async () => {
    connectBrowser('r-1', 'k-routes');
    const res = await routeThrough(pool, 'GET', '/pool');
    assert.equal(res.body.size, 1);
  });

  it('shows the cookie jar of the caller’s default persona', async () => {
    const res = await routeThrough(pool, 'GET', '/pool/cookies');
    assert.equal(res.body.persona, container.personas.defaultFor('k-routes').id);
    assert.ok(Array.isArray(res.body.cookies));
  });
});
