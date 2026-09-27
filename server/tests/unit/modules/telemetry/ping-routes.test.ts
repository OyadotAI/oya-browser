/**
 * Unit tests for the ping self-hosted servers send the hosted deployment: what
 * it accepts and records, what it answers, how often one address may send it,
 * and that only the hosted deployment takes it.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import express from 'express';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-ping-routes-');
const { router } = await import('../../../../src/modules/telemetry/ping-routes.ts');
const { track } = await import('../../../../src/modules/telemetry/index.ts');
const { sendError } = await import('../../../../src/platform/errors.ts');
const limits = await import('../../../../src/platform/limits.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');

/** A ping as the licensing module sends it. */
const PING = {
  install_id: '6ed27761-8487-470f-b5ae-81d1886af2b4',
  version: '1.0.135',
  browsers: 3,
  peak_cloud: 7,
  license_id: null,
};

/** Posts `body` to the ping route on a throwaway app; answers the status and JSON. */
async function post(body: unknown) {
  const app = express().use(express.json(), router, (err, req, res, _next) => sendError(res, err, req));
  const server = app.listen(0);
  try {
    const port = (server.address() as any).port;
    const res = await fetch(`http://127.0.0.1:${port}/telemetry/ping`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    });
    return { status: res.status, body: await res.json() };
  } finally {
    server.close();
  }
}

describe('POST /telemetry/ping', () => {
  beforeEach(async () => {
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    limits.reset();
    await getConnection().delete('installs', {});
  });
  afterEach(() => {
    delete process.env.STRIPE_SECRET_KEY;
    mock.restoreAll();
  });

  it('records the install and counts it by its install id, saying an unlicensed install is not revoked', async () => {
    const seen = mock.method(track, 'installPinged', () => {});
    assert.deepEqual(await post(PING), { status: 200, body: { revoked: false } });
    assert.deepEqual(seen.mock.calls[0].arguments, [
      PING.install_id,
      { version: '1.0.135', browsers: 3, peak_cloud: 7, licensed: false },
    ]);
    const [install] = await getConnection().select('installs', { install_id: PING.install_id });
    assert.equal(install.peak_cloud, 7);
    assert.equal(install.pings, 1);
  });

  it('counts an install as an event once a day, however often it pings', async () => {
    const seen = mock.method(track, 'installPinged', () => {});
    await post(PING);
    await post({ ...PING, peak_cloud: 9 });
    assert.equal(seen.mock.calls.length, 1);
    const [install] = await getConnection().select('installs', { install_id: PING.install_id });
    assert.equal(install.pings, 2);
    assert.equal(install.peak_cloud, 9);
  });

  it('tells an install its license is revoked once an admin revoked it', async () => {
    mock.method(track, 'installPinged', () => {});
    const license = {
      licensee: 'A',
      max_concurrent: 9,
      expires_at: '2030-01-01T00:00:00.000Z',
      created_at: '2026-01-01T00:00:00.000Z',
    };
    await getConnection().upsert('licenses', [
      { id: 'lic-bad', ...license, revoked_at: '2026-02-01T00:00:00.000Z' },
      { id: 'lic-good', ...license, revoked_at: null },
    ]);
    assert.deepEqual((await post({ ...PING, license_id: 'lic-bad' })).body, { revoked: true });
    assert.deepEqual((await post({ ...PING, license_id: 'lic-good' })).body, { revoked: false });
  });

  it('refuses a ping with a field it cannot trust, naming the field', async () => {
    mock.method(track, 'installPinged', () => {});
    for (const [field, value] of [
      ['install_id', 'nope'],
      ['version', 'x'.repeat(40)],
      ['browsers', -1],
      ['peak_cloud', 1.5],
      ['license_id', 'a b'],
    ]) {
      limits.reset();
      const { status, body } = await post({ ...PING, [field]: value });
      assert.equal(status, 400, field);
      assert.equal(body.field, field);
    }
  });

  it('refuses an address that pings more than a few times an hour', async () => {
    mock.method(track, 'installPinged', () => {});
    const answers = [];
    for (let i = 0; i < 6; i++) answers.push((await post(PING)).status);
    assert.deepEqual(answers, [200, 200, 200, 200, 429, 429]);
  });

  it('is not there on a self-hosted server', async () => {
    delete process.env.STRIPE_SECRET_KEY;
    assert.equal((await post(PING)).status, 404);
  });
});
