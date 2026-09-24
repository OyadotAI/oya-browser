/**
 * The real pg shim against a real Postgres, exercising the same contract the
 * psql-stubbed test-control-postgres.js covers, but through the code path a
 * DATABASE_URL deployment actually uses.
 *
 * Requires an isolated database with the migrations applied, and OYA_TEST_LIVE=1
 * so the hermetic preload leaves DATABASE_URL alone:
 *   DATABASE_URL=postgres://... node server/migrations/run.mjs
 *   OYA_TEST_LIVE=1 DATABASE_URL=postgres://... node --import ./tests/support/hermetic.js tests/integration/control-pg-client.test.js
 */
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';

process.env.OYA_PROFILE_SECRET ||= 'pg-client-contract-tests';
process.env.OYA_STORAGE = 'postgres';
if (!process.env.DATABASE_URL) {
  console.log('test-control-pg-client: DATABASE_URL not set, skipping.');
  process.exit(0);
}

const { pgRemote, closePgPool } = await import('../../src/modules/control/pg-client.ts');
const { ControlStore } = await import('../../src/modules/control/store.ts');
const { ControlService } = await import('../../src/modules/control/service.ts');

const remote = pgRemote();
assert.ok(remote, 'pgRemote() must be available when DATABASE_URL is set');

// Two services over one database: the point of Postgres here is many replicas.
const a = new ControlService(new ControlStore({ remote }));
const b = new ControlService(new ControlStore({ remote }));
const key = `pg-client-${randomUUID()}`;

// A jsonb array argument: node-postgres would render a JS array as a Postgres
// array literal, so this is the case that breaks if toParam() stops stringifying.
const attempts = await Promise.allSettled(
  Array.from({ length: 8 }, (_, i) => (i % 2 ? a : b).reserve(key, { provider: 'cdp', maxConcurrent: 1 })),
);
assert.equal(attempts.filter((x) => x.status === 'fulfilled').length, 1, 'one shared slot across replicas');
assert.equal(attempts.filter((x) => x.status === 'rejected' && x.reason.code === 'quota_exceeded').length, 7);

const session = attempts.find((x) => x.status === 'fulfilled').value;
await a.update(key, session.id, { state: 'ready' });

// Command gating, so control_begin/control_finish round-trip (text + bigint args).
const end = await a.beginCommand(session.id);
await assert.rejects(b.takeover(key, session.id, 'acquire', 'operator'), { code: 'commands_pending' });
await b.takeover(key, session.id, 'request', 'operator');
await assert.rejects(a.beginCommand(session.id), { code: 'control_paused' });
await assert.rejects(a.takeover(key, session.id, 'acquire', 'another-operator'), { code: 'control_busy' });
await end();
await b.takeover(key, session.id, 'acquire', 'operator');
await assert.rejects(a.beginCommand(session.id), { code: 'control_paused' });

// Nullable named arguments (actor => null) must not confuse type inference.
const events = await a.store.backend.events({ project: null, after: 0, limit: 10 });
assert.ok(Array.isArray(events), 'control_read_events returns rows through the shim');

// Credentials, to prove a non-array jsonb write and a read-back.
const cred = await a.credential(key, { role: 'viewer' });
assert.equal((await b.authenticate(cred.token)).role, 'viewer');
await b.revoke(key, cred.id);
await assert.rejects(a.authenticate(cred.token), { status: 401 });

await a.update(key, session.id, { state: 'stopped' });

// Idempotency across replicas: one reservation, not two.
const same = await Promise.all(
  [a, b].map((c) =>
    c.reserve(key, {
      provider: 'cdp',
      idempotencyKey: 'shim-same',
      request: { name: "quoted ' safely" },
    }),
  ),
);
assert.equal(same[0].id, same[1].id, 'idempotency key is honoured through the shim');

await closePgPool();
console.log(
  'pg shim contracts passed: pooling, jsonb arrays, named + null args, admission race, gating, credentials, idempotency.',
);
