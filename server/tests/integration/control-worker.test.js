/**
 * Control-plane worker contracts against a throwaway SQLite data dir: cleanup
 * retry and release, signed webhook delivery with retry and a lease against
 * concurrent workers, attach failures, dead-replica routing, reconnect
 * capacity, cross-replica enrollment, budgets versus the queue, metered spend
 * that survives pruning, no-op commits, and egress humanHosts enforcement.
 */
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHmac } from 'node:crypto';
import http from 'node:http';
const directory = await mkdtemp(join(tmpdir(), 'oya-worker-test-'));
Object.assign(process.env, {
  OYA_DATA_DIR: directory,
  OYA_PROFILE_SECRET: 'worker-contract',
  API_KEYS: 'worker-owner',
});
delete process.env.SUPABASE_URL;
delete process.env.SUPABASE_SERVICE_KEY;
const { control, hash, projectId } = await import('../../src/modules/control/service.ts');
const { tick, deliver, maintainControl, stopWorkers } = await import('../../src/modules/control/worker.ts');
const { ownerFor } = await import('../../src/modules/control/cluster.ts');
const { createEgressServer } = await import('../../src/modules/control/egress.ts');
const service = control();
const patch = (kind, id, changes) =>
  service.store.transact(async (tx) => {
    Object.assign(await tx.get(kind, id), changes);
  });
try {
  const key = 'worker-owner';
  const session = await service.reserve(key, { provider: 'missing-vendor', maxConcurrent: 1 });
  await service.update(key, session.id, { state: 'cleanup_pending', provisioningActive: false });
  await tick(service);
  const pending = (await service.read(key)).sessions.find((s) => s.id === session.id);
  assert.equal(pending.state, 'cleanup_pending');
  assert.equal(pending.cleanupAttempts, 1);
  await assert.rejects(service.reserve(key, { provider: 'cdp', maxConcurrent: 1 }), { code: 'quota_exceeded' });
  // A verified attached-only CDP session has no vendor resource to delete.
  await service.update(key, session.id, { provider: 'cdp', nextCleanupAt: 0 });
  await tick(service);
  assert.equal((await service.read(key)).sessions.find((s) => s.id === session.id).state, 'stopped');
  const hook = await service.webhook(key, { url: 'https://example.com/events', types: ['project.settings.updated'] });
  await service.settings(key, { recordingDays: 2 });
  let calls = 0;
  const delivery = async (url, body, headers) => {
    calls++;
    const [time, signature] = headers['Oya-Signature'].split(',');
    assert.equal(
      signature,
      `v1=${createHmac('sha256', hook.secret)
        .update(`${time.slice(2)}.${body}`)
        .digest('hex')}`,
    );
    assert.equal(String(JSON.parse(body).id), headers['Oya-Event-Id']);
    assert.equal(JSON.parse(body).type, 'project.settings.updated');
    return calls > 1;
  };
  await deliver(service, delivery);
  let event = (await service.read(key)).deliveries[0];
  assert.equal(event.state, 'pending');
  assert.equal(event.attempts, 1);
  await patch('delivery', event.id, { nextAt: 0 });
  await Promise.all([deliver(service, delivery), deliver(service, delivery)]);
  event = await service.store.get('delivery', event.id);
  assert.equal(event.state, 'delivered');
  assert.equal(calls, 2, 'delivery lease excludes concurrent workers');

  const attached = await service.reserve('r1', { provider: 'cdp' });
  assert.equal(
    (await service.complete('r1', attached.id, 502, { error: 'unreachable' })).state,
    'failed',
    'a failed attach frees its slot',
  );
  const orphan = await service.reserve('r1', { provider: 'cdp' });
  await patch('session', orphan.id, { instance: 'replaced-process', leaseUntil: 0 });
  assert.equal(await ownerFor(orphan.id, 'r1'), null, 'sessions of a dead replica are served locally');
  const desktop = await service.reserve('r2', { provider: 'oya-desktop', maxConcurrent: 1 });
  await service.update('r2', desktop.id, { state: 'disconnected' });
  await service.reserve('r2', { provider: 'oya-desktop', maxConcurrent: 1 });
  await assert.rejects(
    service.adopt('r2', { id: desktop.id, provider: 'oya-desktop', maxConcurrent: 1 }),
    { code: 'quota_exceeded' },
    'reconnect rechecks capacity',
  );
  const enrolling = await service.reserve('r3', { provider: 'oya-selfhosted' });
  await patch('session', enrolling.id, { instance: 'provisioner' });
  assert.equal(
    (await service.adopt('r3', { id: enrolling.id, provider: 'oya-selfhosted' })).state,
    'ready',
    'enrollment may land on any replica',
  );
  const legacy = await service.reserve('r4', { provider: 'legacy' });
  await service.update('r4', legacy.id, { state: 'cleanup_pending', provisioningActive: false });
  assert.equal(
    (await service.cancel('r4', legacy.id, { force: true })).state,
    'stopped',
    'force reconciles an undeletable resource',
  );

  await service.settings('r5', { budgetUsd: 1, rates: { 'oya-selfhosted': 6 }, maxConcurrent: 1 });
  const paid = await service.reserve('r5', { provider: 'oya-selfhosted', managed: true });
  const waiting = await service.reserve('r5', {
    provider: 'oya-selfhosted',
    managed: true,
    request: { queueMs: 60000 },
  });
  await patch('project', projectId('r5'), { costUsd: 5 });
  await patch('session', paid.id, { provisioningActive: false });
  await tick(service);
  const budgeted = (await service.read('r5')).sessions;
  assert.equal(budgeted.find((x) => x.id === paid.id).state, 'cleanup_pending');
  assert.equal(
    budgeted.find((x) => x.id === waiting.id).state,
    'queued',
    'budget exhaustion leaves queued work to its deadline',
  );

  // Metering accrues into the project, so pruning a finished session keeps its spend.
  await service.settings('r6', { budgetUsd: 10, rates: { 'oya-selfhosted': 60 } });
  const metered = await service.reserve('r6', { provider: 'oya-selfhosted', managed: true });
  await patch('session', metered.id, { meteredAt: Date.now() - 10 * 60000, provisioningActive: false });
  await tick(service);
  assert.ok(Math.abs((await service.project('r6')).costUsd - 10) < 0.1, 'ten minutes at $60 per hour');
  await patch('session', metered.id, { state: 'stopped', updatedAt: Date.now() - 8 * 86400000 });
  await maintainControl(service);
  assert.equal(await service.store.get('session', metered.id), null, 'terminal sessions are pruned after seven days');
  await assert.rejects(
    service.reserve('r6', { provider: 'oya-selfhosted', managed: true }),
    { code: 'budget_admission' },
    'pruned spend still counts',
  );
  // Stopping bills the interval since the last metering, whichever code path stops the session.
  await service.settings('r8', { rates: { cdp: 60 } });
  const brief = await service.reserve('r8', { provider: 'cdp' });
  await patch('session', brief.id, { meteredAt: Date.now() - 6 * 60000 });
  await service.update('r8', brief.id, { state: 'stopped' });
  assert.ok(Math.abs((await service.project('r8')).costUsd - 6) < 0.1, 'the final interval is billed on stop');
  const version = async () => (await service.store.load([{ kind: 'project', id: projectId('r6') }]))[0][0].version;
  const before = await version();
  await service.store.transact(async (tx) => {
    await tx.get('project', projectId('r6'));
  });
  assert.equal(await version(), before, 'no-op transactions do not commit');

  const governed = await service.reserve('r7', { provider: 'oya-selfhosted' });
  await service.update('r7', governed.id, {
    state: 'ready',
    managed: true,
    egressHash: hash('t'),
    policies: [{ humanHosts: ['example.com'] }],
  });
  const proxy = createEgressServer();
  await new Promise((r) => proxy.listen(0, '127.0.0.1', r));
  const status = await new Promise((resolve, reject) => {
    const req = http.request({
      host: '127.0.0.1',
      port: proxy.address().port,
      method: 'CONNECT',
      path: 'example.com:443',
      headers: { 'Proxy-Authorization': `Basic ${Buffer.from(`${governed.id}:t`).toString('base64')}` },
    });
    req.on('connect', (res) => {
      res.socket.destroy();
      resolve(res.statusCode);
    });
    req.on('error', reject);
    req.end();
  });
  proxy.close();
  assert.equal(status, 403, 'egress enforces humanHosts while the agent has control');
  console.log(
    'Worker contracts passed: cleanup retry and release, signed webhook retry, concurrent delivery exclusion, attach failures, dead-owner routing, reconnect capacity, cross-replica enrollment, reconciliation, budget vs queue, metered spend survives pruning, no-op commits, egress humanHosts.',
  );
} finally {
  await stopWorkers();
  await service.store.close();
  await rm(directory, { recursive: true, force: true });
}
process.exit(0);
