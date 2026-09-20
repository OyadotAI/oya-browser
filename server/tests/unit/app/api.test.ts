/**
 * Unit tests for the API router's own behaviour (api.ts): HTTP metrics
 * labelled by route pattern, the JSON error handler, and the process-wide
 * registry listeners for CDP live view and sign-in tallies. Routes are driven
 * through the Express router with a fake request.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { router } from '../../../src/app/api.ts';
import { registry } from '../../../src/modules/browsers/registry.ts';
import { reset as resetMetrics, snapshot } from '../../../src/platform/metrics.ts';
import { HttpError } from '../../../src/platform/errors.ts';
import { connectBrowser, disconnectBrowser } from '../support/fakes.ts';
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

  it('hides the message of an unexpected error behind a 503', () => {
    const res = new FakeResponse();
    errorHandler(new Error('db password is hunter2'), {}, res, () => {});
    assert.deepEqual(
      [res.statusCode, res.body],
      [503, { error: 'Operation could not be completed', code: 'operation_failed' }],
    );
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

describe('registry listeners', () => {
  afterEach(() => disconnectBrowser(B));

  it('starts a CDP browser’s screencast for its first viewer and pushes its frames', async () => {
    const driver: any = driveBrowser(B, () => ({ ok: true }));
    let onFrame;
    driver.startScreencast = mock.fn(async (cb) => (onFrame = cb));
    driver.stopScreencast = mock.fn(async () => {});
    const viewer = new FakeResponse();
    registry.addViewer(B, viewer);
    onFrame('frame-1');
    assert.equal(registry.get(B).lastFrame, 'frame-1');
    registry.removeViewer(B, viewer);
    assert.equal(driver.stopScreencast.mock.callCount(), 1);
  });

  it('leaves an Oya browser, which pushes its own frames, alone', () => {
    connectBrowser(B);
    assert.doesNotThrow(() => registry.addViewer(B, new FakeResponse()));
  });
});
