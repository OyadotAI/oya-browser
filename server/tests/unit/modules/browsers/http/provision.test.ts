/**
 * Unit tests for the Oya Cloud provisioning routes: refused while draining,
 * over the hourly quota or unconfigured, and a sandbox delete that reports
 * its failure.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { deleteSandbox, provision } from '../../../../../src/modules/browsers/http/provision.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { isConfigured as sandboxConfigured } from '../../../../../src/drivers/sandbox.ts';
import { QUOTAS } from '../../../../../src/platform/limits.ts';
import * as usage from '../../../../../src/platform/usage.ts';
import { FakeResponse, fakeRequest } from '../../../support/browsers.ts';

/** Calls provision with `body` and returns the response. */
async function launch(body: object = {}) {
  const res = new FakeResponse();
  await provision(fakeRequest({ key: 'k-prov', body }), res);
  return res;
}

describe('provision', () => {
  afterEach(() => {
    registry.draining = undefined;
    mock.restoreAll();
  });

  it('answers 503 while the server is draining', async () => {
    registry.draining = true;
    const res = await launch();
    assert.deepEqual([res.statusCode, res.body], [503, { error: 'Server is draining' }]);
  });

  it('answers 429 once the key has launched its hourly quota', async () => {
    usage.reset();
    usage.record('k-prov', 'sandboxes_created', QUOTAS.sandboxesPerHour);
    const res = await launch();
    assert.equal(res.statusCode, 429);
    assert.match(res.body.error, /Sandbox quota reached for this hour/);
    usage.reset();
  });

  it('answers 409 naming what is missing when Oya Cloud is not configured', { skip: sandboxConfigured() }, async () => {
    const res = await launch({ count: 5 });
    assert.equal(res.statusCode, 409);
    assert.ok(Array.isArray(res.body.missing));
  });
});

describe('deleteSandbox', () => {
  afterEach(() => mock.restoreAll());

  it('answers the failure’s status when the sandbox cannot be removed', { skip: sandboxConfigured() }, async () => {
    mock.method(console, 'error', () => {});
    const res = new FakeResponse();
    await deleteSandbox(fakeRequest({ params: { browserId: 'b-x' } }), res);
    assert.equal(res.statusCode, 409);
    assert.ok(res.body.error);
  });
});
