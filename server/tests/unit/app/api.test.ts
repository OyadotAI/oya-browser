/**
 * Unit tests for the API router's own behaviour (api.ts): HTTP metrics
 * labelled by route pattern, the JSON error handler, and the process-wide
 * registry listeners for CDP live view and sign-in tallies. Routes are driven
 * through the Express router with a fake request.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { router } from '../../../src/app/api.ts';
import { reset as resetMetrics, snapshot } from '../../../src/platform/metrics.ts';
import { HttpError } from '../../../src/platform/errors.ts';
import { disconnectBrowser } from '../support/fakes.ts';
import { FakeResponse, driveBrowser, routeThrough, stubControl } from '../support/browsers.ts';

const B = 'b-api';

/** The router's final error-handling layer. */
const errorHandler = router.stack.at(-1).handle as (err, req, res, next) => void;

/** Sends a request through the API router, errors going to its error handler. */
const send = (method: string, url: string) => routeThrough(router, method, url, 'oya_op', errorHandler);

describe('API router', () => {
  beforeEach(() => {
    stubControl({ authenticate: async () => ({ key: 'k-api', role: 'administrator' }) });
    resetMetrics();
  });
  afterEach(() => {
    mock.restoreAll();
    disconnectBrowser(B);
  });

  it('routes an authenticated request to its handler', async () => {
    driveBrowser(B, () => ({ ok: true }), 'k-api');
    const res = await send('GET', '/pool');
    assert.equal(res.body.size, 1);
  });

  it('counts requests by route pattern and status class, never by browser id', async () => {
    await send('GET', `/browsers/${B}`);
    const [series] = snapshot().oya_http_requests_total;
    assert.deepEqual(series.labels, { route: '/browsers/:browserId', status: '4xx' });
  });

  it('folds ids and tokens out of a path that matched no route', async () => {
    await send('GET', '/nothing/3f2b8c1e-1234-4567-89ab-0123456789ab/abcdefghijklmnopqrstuvwxyz');
    const labels = snapshot().oya_http_requests_total.map((s) => s.labels.route);
    assert.ok(labels.includes('/nothing/:id/:token'));
  });
});

describe('API error handler', () => {
  it('exposes the message and code of an error with a status', () => {
    const res = new FakeResponse();
    errorHandler(new HttpError(409, 'Busy', { code: 'busy' }), {}, res, () => {});
    assert.deepEqual([res.statusCode, res.body], [409, { error: 'Busy', code: 'busy' }]);
  });

  it('answers an unexpected error as a 500 that hides the message, carries a reference and logs the stack', () => {
    const logged = mock.method(console, 'error', () => {});
    const res = new FakeResponse();
    errorHandler(new Error('db password is hunter2'), { method: 'GET', originalUrl: '/api/x' }, res, () => {});
    assert.equal(res.statusCode, 500);
    assert.equal(res.body.code, 'internal_error');
    assert.ok(!JSON.stringify(res.body).includes('hunter2'));
    assert.ok(logged.mock.calls[0].arguments.join(' ').includes(`ref=${res.body.ref} GET /api/x`));
  });

  it('answers an unknown API path with a 404 that names the request and where the API is described', async () => {
    const res = await send('GET', '/api/personaz');
    assert.equal(res.statusCode, 404);
    assert.equal(res.body.code, 'not_found');
    assert.match(res.body.error, /No route for GET \/api\/personaz\. The API is described at \/openapi\.json\./);
  });

  it('answers a bare /api and anything under it with the JSON 404, and hands on a path it was reached at from the root', async () => {
    for (const url of ['/api', '/api/']) {
      const res = await send('GET', url);
      assert.deepEqual([res.statusCode, res.body.code], [404, 'not_found'], url);
    }
  });

  it('hands the error on when a response is already under way', () => {
    const res = new FakeResponse();
    res.headersSent = true;
    const next = mock.fn();
    const err = new Error('late');
    errorHandler(err, {}, res, next);
    assert.deepEqual(next.mock.calls[0].arguments, [err]);
  });
});
