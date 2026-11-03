/** Exercises Keel's unauthenticated receiver, durable rows and refusal paths through HTTP. */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import express from 'express';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-keel-ping-');
const { router } = await import('../../../../src/modules/telemetry/keel-routes.ts');
const { router: browserRouter } = await import('../../../../src/modules/telemetry/ping-routes.ts');
const { sendError } = await import('../../../../src/platform/errors.ts');
const limits = await import('../../../../src/platform/limits.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');

const PING = {
  install_id: '6ed27761-8487-470f-b5ae-81d1886af2b4',
  version: '0.4.0',
  os: 'macos',
  day: '2026-10-11',
  steps: 200,
  license_id: null,
};

/** Posts to the API-mounted routes with no credentials or real outbound traffic. */
async function post(body: unknown, path = '/api/telemetry/keel/ping', routes = router) {
  const app = express().use(express.json()).use('/api', routes, browserRouter);
  app.use((err, req, res, _next) => sendError(res, err, req));
  const server = app.listen(0);
  try {
    const port = (server.address() as any).port;
    const res = await fetch(`http://127.0.0.1:${port}${path}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    });
    return { status: res.status, body: await res.json() };
  } finally {
    server.close();
  }
}

describe('POST /api/telemetry/keel/ping', () => {
  beforeEach(async () => {
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    limits.reset();
    await getConnection().delete('keel_usage', {});
    await getConnection().delete('keel_installs', {});
    await getConnection().delete('installs', {});
  });
  afterEach(() => {
    delete process.env.STRIPE_SECRET_KEY;
    mock.restoreAll();
  });

  it('records only anonymous report fields separately from Oya Browser installs', async () => {
    assert.deepEqual(await post({ ...PING, path: '/private/repo' }), { status: 200, body: { ok: true } });
    const [row] = await getConnection().select('keel_usage', {});
    const { received_at, runtime_ms, project_ids, ...report } = row;
    assert.deepEqual(report, PING);
    assert.equal(runtime_ms, null);
    assert.equal(project_ids, null);
    assert.ok(Number.isFinite(Date.parse(received_at)));
    assert.deepEqual(await getConnection().select('installs', {}), []);
  });

  it('is reachable from the production API router without authentication', async () => {
    const { router: api } = await import('../../../../src/app/api.ts');
    assert.equal((await post(PING, '/api/telemetry/keel/ping', api)).status, 200);
  });

  it('deduplicates concurrent reports and preserves the first count', async () => {
    const answers = await Promise.all([post(PING), post(PING)]);
    assert.ok(answers.every((a) => a.status === 200));
    await post({ ...PING, steps: 201 });
    const rows = await getConnection().select('keel_usage', {});
    assert.equal(rows.length, 1);
    assert.equal(rows[0].steps, 200);
  });

  it('keeps reports for different days and installs, including the first-run marker', async () => {
    for (const body of [
      PING,
      { ...PING, day: '2026-10-12', steps: 4294967295, license_id: 'lic-1' },
      { ...PING, day: '', steps: 0 },
      { ...PING, install_id: '2ed27761-8487-470f-b5ae-81d1886af2b4' },
    ])
      assert.equal((await post(body)).status, 200);
    const rows = await getConnection().select('keel_usage', {});
    assert.equal(rows.length, 4);
    assert.equal(rows.find((r) => r.day === '2026-10-12').steps, 4294967295);
  });

  it('refuses malformed reports without storing anything', async () => {
    const answer = await post({ ...PING, steps: -1 });
    assert.equal(answer.status, 400);
    assert.equal(answer.body.field, 'steps');
    assert.deepEqual(await getConnection().select('keel_usage', {}), []);
  });

  it('validates the entire v2 batch before storing its first day', async () => {
    const { day, steps, ...identity } = PING;
    const report = { day, steps, runtime_ms: 1000, project_ids: ['a'.repeat(64)] };
    const body = { schema_version: 2, ...identity, reports: [report, { ...report, day: 'not-a-day' }] };
    assert.equal((await post(body)).status, 400);
    assert.deepEqual(await getConnection().select('keel_usage'), []);
    assert.deepEqual(await getConnection().select('keel_installs'), []);
  });

  it('accepts a measured batch and separately accepts presence without inventing daily usage', async () => {
    const { day, steps, ...identity } = PING;
    const body = { schema_version: 2, ...identity, reports: [{ day, steps, runtime_ms: 1000, project_ids: [] }] };
    assert.equal((await post(body)).status, 200);
    assert.equal((await post({ ...body, license_id: null, reports: [] })).status, 200);
    assert.equal((await getConnection().select('keel_usage')).length, 1);
    assert.equal((await getConnection().select('keel_installs')).length, 1);
  });

  it('rate limits Keel separately from Oya Browser pings', async () => {
    for (let i = 0; i < 4; i++) assert.equal((await post(PING)).status, 200);
    assert.equal((await post(PING)).status, 429);
    const browser = { install_id: PING.install_id, version: '1.0.0', browsers: 0, peak_cloud: 0, license_id: null };
    assert.equal((await post(browser, '/api/telemetry/ping')).status, 200);
  });

  it('does not accept reports on self-hosted servers', async () => {
    delete process.env.STRIPE_SECRET_KEY;
    assert.equal((await post(PING)).status, 404);
    assert.deepEqual(await getConnection().select('keel_usage', {}), []);
  });

  it('does not acknowledge a report the database failed to store', async () => {
    mock.method(getConnection(), 'upsert', async () => {
      throw new Error('database unavailable');
    });
    assert.equal((await post(PING)).status, 500);
  });
});
