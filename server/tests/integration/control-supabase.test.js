/**
 * Live smoke test of the control-plane RPC contract through real PostgREST, as the server calls it.
 * Needs SUPABASE_URL and SUPABASE_SERVICE_KEY with migration 008 applied:
 *   node --env-file=server/.env server/test-control-supabase.js
 * Everything it creates lives in a throwaway project and is deleted at the end, including on failure.
 */
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
const { db } = await import('../../src/platform/db.ts');
if (!db) throw new Error('Set SUPABASE_URL and SUPABASE_SERVICE_KEY');
const { ControlStore } = await import('../../src/modules/control/store.ts');
const { ControlService, projectId } = await import('../../src/modules/control/service.ts');
const a = new ControlService(new ControlStore({ remote: db })),
  b = new ControlService(new ControlStore({ remote: db }));
const key = `smoke-${randomUUID()}`,
  project = projectId(key);
try {
  const attempts = await Promise.allSettled(
    Array.from({ length: 6 }, (_, i) => (i % 2 ? a : b).reserve(key, { provider: 'cdp', maxConcurrent: 1 })),
  );
  assert.equal(attempts.filter((x) => x.status === 'fulfilled').length, 1, 'two stores share one slot');
  const session = attempts.find((x) => x.status === 'fulfilled').value;
  await a.update(key, session.id, { state: 'ready' });
  const end = await a.beginCommand(session.id);
  await assert.rejects(b.takeover(key, session.id, 'acquire', 'smoke'), { code: 'commands_pending' });
  await end();
  assert.equal((await b.takeover(key, session.id, 'acquire', 'smoke')).mode, 'human');
  const credential = await a.credential(key, { role: 'viewer' });
  assert.equal((await b.authenticate(credential.token)).role, 'viewer');
  await b.revoke(key, credential.id);
  await assert.rejects(a.authenticate(credential.token), { status: 401 });
  await a.webhook(key, { url: 'https://example.com/smoke', types: ['project.settings.updated'] });
  await b.settings(key, { recordingDays: 3 });
  const [delivery] = await a.store.list('delivery', { project, states: ['pending'] });
  assert.equal(
    (await b.store.events({ seqs: [delivery.eventSeq] }))[0].type,
    'project.settings.updated',
    'delivery created with its event',
  );
  const events = await a.events(key);
  assert.equal((await a.events(key, { after: events.at(-2).id })).length, 1, 'event cursor');
  assert.equal((await a.read(key)).sessions.length, 1);
  console.log(
    'Supabase smoke passed: RPC permissions, shared admission, command gate vs takeover, credentials, event fan-out, cursor.',
  );
} finally {
  // Remove this run's rows (sessions take their gates with them) and events.
  const kinds = ['session', 'credential', 'webhook', 'delivery', 'idempotency', 'ticket'];
  const found = await a.store.load(kinds.map((kind) => ({ kind, project })));
  await a.store.transact(async (tx) => {
    for (const [i, rows] of found.entries()) for (const row of rows) await tx.delete(kinds[i], row.id);
    await tx.delete('project', project);
  });
  await a.store.prune(Date.now(), { [project]: Date.now() + 1 });
  const left =
    (await a.store.load([...kinds.map((kind) => ({ kind, project })), { kind: 'project', id: project }])).flat()
      .length + (await a.store.events({ project })).length;
  console.log(left ? `Cleanup incomplete: ${left} rows or events remain for ${project}` : `Cleaned up ${project}.`);
}
