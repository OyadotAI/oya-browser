/** Requires an isolated database with migration 008 applied. Never resets database state. */
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { randomUUID } from 'node:crypto';
process.env.OYA_PROFILE_SECRET ||= 'postgres-control-contract-tests';
const { ControlStore } = await import('./src/control/store.js');
const { ControlService, projectId } = await import('./src/control/service.js');
const exec = promisify(execFile);
const sql = async query => (await exec('psql', ['-XqAt', '-v', 'ON_ERROR_STOP=1', '-d', process.env.PGDATABASE || 'postgres', '-c', query], { maxBuffer: 8 * 1024 * 1024 })).stdout.trim();
const literal = v => v === null || v === undefined ? 'null' : typeof v === 'object' ? `'${JSON.stringify(v).replaceAll("'", "''")}'::jsonb` : typeof v === 'string' ? `'${v.replaceAll("'", "''")}'` : String(v);
// Stands in for PostgREST: named-argument calls as the service role.
const remote = { async rpc(name, args = {}) {
  try {
    const raw = await sql(`set role service_role; select oya_browser.${name}(${Object.entries(args).map(([k, v]) => `${k} => ${literal(v)}`).join(', ')});`);
    return { data: raw === '' ? null : JSON.parse(raw) };
  } catch (error) { return { error: { message: String(error.stderr || error.message) } }; }
} };
const a = new ControlService(new ControlStore({ remote }));
const b = new ControlService(new ControlStore({ remote }));
const key = `pg-test-${randomUUID()}`;
await sql('grant usage on schema oya_browser to service_role');
const attempts = await Promise.allSettled(Array.from({ length: 12 }, (_, i) => (i % 2 ? a : b).reserve(key, { provider: 'cdp', maxConcurrent: 1 })));
assert.equal(attempts.filter(x => x.status === 'fulfilled').length, 1, 'two replicas respect one shared slot');
assert.equal(attempts.filter(x => x.status === 'rejected' && x.reason.code === 'quota_exceeded').length, 11);
const session = attempts.find(x => x.status === 'fulfilled').value;
await a.update(key, session.id, { state: 'ready' });
await assert.rejects(a.store.beginCommand(session.id, null, 'stale-replica'), { code: 'control_paused' });
const end = await a.beginCommand(session.id);
await assert.rejects(b.takeover(key, session.id, 'acquire', 'operator'), { code: 'commands_pending' });
await b.takeover(key, session.id, 'request', 'operator');
await assert.rejects(a.beginCommand(session.id), { code: 'control_paused' });
await assert.rejects(a.takeover(key, session.id, 'acquire', 'another-operator'), { code: 'control_busy' });
await end();
await b.takeover(key, session.id, 'acquire', 'operator');
await assert.rejects(a.beginCommand(session.id), { code: 'control_paused' });
const c = await a.credential(key, { role: 'viewer' });
assert.equal((await b.authenticate(c.token)).role, 'viewer');
await b.revoke(key, c.id);
await assert.rejects(a.authenticate(c.token), { status: 401 });
await a.update(key, session.id, { state: 'stopped' });
const idempotent = await Promise.all([a, b].map(c => c.reserve(key, { provider: 'cdp', idempotencyKey: 'same', request: { name: "quoted ' safely" } })));
assert.equal(idempotent[0].id, idempotent[1].id);
await b.update(key, idempotent[0].id, { state: 'stopped' });
await a.webhook(key, { url: 'https://example.com/hook', types: ['project.settings.updated'] });
await b.settings(key, { recordingDays: 3 });
const [delivery] = await b.store.list('delivery', { project: projectId(key), states: ['pending'] });
assert.equal((await a.store.events({ seqs: [delivery.eventSeq] }))[0].type, 'project.settings.updated', 'deliveries are created atomically with their event');
assert.ok((await a.events(key)).length > 5);
assert.equal((await a.events(key, { after: (await a.events(key)).at(-2).id })).length, 1, 'event cursor');
await a.store.prune(Date.now(), { [projectId(key)]: Date.now() + 1 });
assert.equal((await a.events(key)).length, 0, 'retention deletes events');
await assert.rejects(sql('set role anon; select oya_browser.control_load(\'[]\'::jsonb);'), /permission denied/);
console.log('Postgres contracts passed: migration, service-role RPC, replica admission race, takeover exclusion, revocation, idempotency, event fan-out, cursor, retention, anonymous denial.');
