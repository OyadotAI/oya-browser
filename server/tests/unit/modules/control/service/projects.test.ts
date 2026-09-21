/**
 * Unit tests for projects: created on a key's first use, the overview with its
 * secrets stripped and other tenants kept out, settings, and the owner's
 * rename and delete.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { projectId } from '../../../../../src/modules/control/service.ts';
import { patchRow, putRow, readySession, scratchService } from '../../../support/control.ts';

const A = 'key-a',
  B = 'key-b';
let service;
beforeEach(() => {
  service = scratchService();
});

/** Makes `user` the owner of the key's project. */
async function own(key, user = 'u-1') {
  await service.project(key);
  await patchRow(service, 'project', projectId(key), { ownerUser: user });
}

describe('project', () => {
  it('creates the key’s project on first use with default settings, without its sealed key', async () => {
    const p = await service.project(A);
    assert.equal(p.id, projectId(A));
    assert.equal(p.name, `Project ${p.id.slice(-6)}`);
    assert.deepEqual(p.settings, {
      recordingDays: 7,
      auditDays: 90,
      budgetUsd: null,
      maxConcurrent: null,
      rates: {},
      policy: {},
    });
    assert.equal('key' in p, false);
  });

  it('creates a project once, announcing it once', async () => {
    await service.project(A);
    await service.project(A);
    const events = await service.events(A);
    assert.deepEqual(
      events.map((e) => e.type),
      ['project.created'],
    );
  });

  it('keeps each key’s project separate', async () => {
    assert.notEqual((await service.project(A)).id, (await service.project(B)).id);
  });

  it('unseals the key a project row holds', async () => {
    await service.project(A);
    assert.equal(service.projectKey(await service.store.get('project', projectId(A))), A);
  });

  it('answers 503 project_key_unavailable when the key cannot be unsealed, including one sealed for another project', async () => {
    await service.project(A);
    await service.project(B);
    const b = await service.store.get('project', projectId(B));
    assert.throws(() => service.projectKey({ ...b, id: projectId(A) }), {
      status: 503,
      code: 'project_key_unavailable',
    });
    assert.throws(() => service.projectKey({ id: 'x', key: 'garbage' }), { code: 'project_key_unavailable' });
  });

  it('refuses a deleted project with 410', async () => {
    await service.project(A);
    await patchRow(service, 'project', projectId(A), { deletedAt: 1 });
    await assert.rejects(service.project(A), { status: 410, code: 'project_deleted' });
    await assert.rejects(service.settings(A, { auditDays: 10 }), { status: 410 });
  });
});

describe('settings', () => {
  it('merges validated changes and records which fields changed', async () => {
    const p = await service.settings(A, { auditDays: 30, rates: { cdp: 1 } });
    assert.equal(p.settings.auditDays, 30);
    assert.equal(p.settings.recordingDays, 7);
    const last = (await service.events(A)).at(-1);
    assert.equal(last.type, 'project.settings.updated');
    assert.deepEqual(last.detail, { fields: ['auditDays', 'rates'] });
  });

  it('refuses an invalid change before touching storage', async () => {
    await assert.rejects(service.settings(A, { auditDays: 0 }), { status: 400 });
    assert.equal(await service.store.get('project', projectId(A)), null);
  });
});

describe('updateOwnedProject', () => {
  it('refuses anyone but the owner with 404', async () => {
    await own(A, 'u-1');
    await assert.rejects(service.updateOwnedProject('u-2', projectId(A), { name: 'x' }), { status: 404 });
    await assert.rejects(service.updateOwnedProject('u-1', 'prj_missing', { name: 'x' }), { status: 404 });
  });

  it('renames with the trimmed name', async () => {
    await own(A);
    assert.deepEqual(await service.updateOwnedProject('u-1', projectId(A), { name: '  Acme  ' }), { ok: true });
    assert.equal((await service.project(A)).name, 'Acme');
    assert.equal((await service.events(A)).at(-1).type, 'project.renamed');
  });

  it('refuses a blank, overlong or missing name', async () => {
    await own(A);
    for (const name of ['   ', 'x'.repeat(101), undefined])
      await assert.rejects(service.updateOwnedProject('u-1', projectId(A), { name }), {
        status: 400,
        code: 'invalid_name',
      });
  });

  it('refuses to delete while browsers are open, saying how many', async () => {
    await own(A);
    await readySession(service, A, 's1');
    await assert.rejects(service.updateOwnedProject('u-1', projectId(A), { remove: true }), {
      status: 409,
      code: 'project_active',
      active: 1,
      message: 'Stop 1 browser before deleting this project',
    });
  });

  it('deletes with stopBrowsers: stops its sessions, revokes its credentials and removes its members', async () => {
    await own(A);
    await readySession(service, A, 's1');
    await service.reserve(A, { id: 's2', provider: 'cdp', cleanup: { kind: 'vendor' } });
    const { token } = await service.credential(A);
    await putRow(service, 'membership', `${projectId(A)}:u-2`, {
      id: `${projectId(A)}:u-2`,
      project: projectId(A),
      userId: 'u-2',
      role: 'viewer',
    });

    await service.updateOwnedProject('u-1', projectId(A), { remove: true, stopBrowsers: true });

    assert.equal((await service.store.get('session', 's1')).state, 'stopped');
    assert.equal(
      (await service.store.get('session', 's2')).state,
      'cleanup_pending',
      'its resource still needs deleting',
    );
    assert.ok((await service.store.get('project', projectId(A))).deletedAt);
    assert.deepEqual(await service.store.list('membership', { project: projectId(A) }), []);
    await assert.rejects(service.authenticate(token), { status: 410 });
    await assert.rejects(service.updateOwnedProject('u-1', projectId(A), { name: 'again' }), { status: 404 });
  });
});

describe('read (overview)', () => {
  it('shows the project’s sessions and events without server-only fields', async () => {
    await service.reserve(A, { id: 's1', provider: 'cdp', cleanup: { kind: 'vendor', secret: 1 } });
    const view = await service.read(A);
    assert.equal(view.project.id, projectId(A));
    assert.equal(view.draining, false);
    assert.equal(view.sessions[0].id, 's1');
    assert.equal('cleanup' in view.sessions[0], false);
    assert.equal('requestHash' in view.sessions[0], false);
    assert.ok(view.events.some((e) => e.type === 'session.provisioning'));
  });

  it('lists people’s credentials without digests, and hides managed-browser credentials', async () => {
    await service.credential(A, { label: 'CI' });
    await service.reserve(A, { id: 's1', provider: 'cdp' });
    await service.enrollmentCredential(A, 's1');
    const { credentials } = await service.read(A);
    assert.deepEqual(
      credentials.map((c) => c.label),
      ['CI'],
    );
    assert.equal('digest' in credentials[0], false);
  });

  it('lists webhooks without their secret, and only deliveries still needing attention', async () => {
    await service.webhook(A, { url: 'https://hooks.example/a', types: ['session.ready', 'session.failed'] });
    await service.emit(A, 'session.ready');
    await service.emit(A, 'session.failed');
    const [first] = await service.store.list('delivery', { project: projectId(A) });
    await patchRow(service, 'delivery', first.id, { state: 'delivered' });
    const view = await service.read(A);
    assert.equal('secret' in view.webhooks[0], false);
    assert.equal(view.deliveries.length, 1);
  });

  it('never shows another project’s sessions, credentials, webhooks or events', async () => {
    await readySession(service, B, 'theirs');
    await service.credential(B);
    await service.webhook(B, { url: 'https://hooks.example/b' });
    const view = await service.read(A);
    assert.deepEqual([view.sessions, view.credentials, view.webhooks, view.deliveries], [[], [], [], []]);
    assert.ok(view.events.every((e) => e.project === projectId(A)));
  });

  it('reports the fleet draining', async () => {
    await service.drain(true);
    assert.equal((await service.read(A)).draining, true);
  });
});
