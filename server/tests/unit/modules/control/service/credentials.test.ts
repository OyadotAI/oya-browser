/**
 * Unit tests for project credentials: issuing (token shown once, digest kept),
 * resolving a token to its principal, expiry, revocation, membership changes,
 * share links bound to one browser, managed-browser enrollment, and tenant
 * isolation between projects.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { hash, projectId } from '../../../../../src/modules/control/service.ts';
import { patchRow, putRow, readySession, scratchService } from '../../../support/control.ts';

const A = 'key-a',
  B = 'key-b',
  NOW = 1_700_000_000_000;
let service;
beforeEach(() => {
  mock.timers.enable({ apis: ['Date'], now: NOW });
  service = scratchService();
});
afterEach(() => mock.timers.reset());

/** A membership row for `user` in the key's project. */
const member = (key, user, role) =>
  putRow(service, 'membership', `${projectId(key)}:${user}`, {
    id: `${projectId(key)}:${user}`,
    project: projectId(key),
    userId: user,
    role,
  });

describe('credential', () => {
  it('issues an operator service account by default and returns its token once', async () => {
    const c = await service.credential(A);
    assert.match(c.token, /^oya_/);
    assert.equal(c.role, 'operator');
    assert.equal(c.label, 'Service account');
    assert.equal('digest' in c, false);
  });

  it('stores only the token’s digest', async () => {
    const { token } = await service.credential(A);
    const stored = await service.store.get('credential', hash(token));
    assert.equal(stored.digest, hash(token));
    assert.equal(JSON.stringify(stored).includes(token), false);
  });

  it('records the issue in the audit log', async () => {
    const c = await service.credential(A, { role: 'viewer' });
    const last = (await service.events(A)).at(-1);
    assert.deepEqual([last.type, last.detail], ['credential.created', { id: c.id, role: 'viewer' }]);
  });

  it('cuts a long label to 100 characters', async () => {
    assert.equal((await service.credential(A, { label: 'x'.repeat(150) })).label.length, 100);
  });

  it('refuses an unknown role and an expiry that is not in the future', async () => {
    await assert.rejects(service.credential(A, { role: 'browser' }), { status: 400, code: 'invalid_role' });
    await assert.rejects(service.credential(A, { expiresAt: NOW }), { status: 400, code: 'invalid_expiry' });
    await assert.rejects(service.credential(A, { expiresAt: 'soon' }), { code: 'invalid_expiry' });
  });

  it('refuses to issue for a member who no longer holds the role', async () => {
    await member(A, 'u-2', 'viewer');
    await assert.rejects(service.credential(A, { role: 'operator' }, 'u-2'), {
      status: 403,
      code: 'membership_changed',
    });
    assert.equal((await service.credential(A, { role: 'viewer' }, 'u-2')).role, 'viewer');
  });
});

describe('authenticate', () => {
  it('resolves a token to its project key, role and ids', async () => {
    const c = await service.credential(A, { role: 'viewer' });
    assert.deepEqual(await service.authenticate(c.token), {
      key: A,
      role: 'viewer',
      project: projectId(A),
      credentialId: c.id,
      sessionId: null,
      memberUser: null,
    });
  });

  it('answers null for a token it has never issued', async () => {
    assert.equal(await service.authenticate('oya_unknown'), null);
  });

  it('refuses a revoked token with 401', async () => {
    const c = await service.credential(A);
    await service.revoke(A, c.id);
    await assert.rejects(service.authenticate(c.token), { status: 401, code: 'revoked_credential' });
  });

  it('refuses a token once it expires', async () => {
    const c = await service.credential(A, { expiresAt: NOW + 1000 });
    assert.ok(await service.authenticate(c.token));
    mock.timers.tick(1000);
    await assert.rejects(service.authenticate(c.token), { status: 401 });
  });

  it('refuses a member’s token once their membership is removed or changed', async () => {
    await member(A, 'u-2', 'operator');
    const c = await service.credential(A, { role: 'operator' }, 'u-2');
    await member(A, 'u-2', 'viewer');
    await assert.rejects(service.authenticate(c.token), { status: 403, code: 'membership_removed' });
  });

  it('treats the owner as the project’s administrator', async () => {
    await service.project(A);
    await patchRow(service, 'project', projectId(A), { ownerUser: 'u-1' });
    const c = await service.credential(A, { role: 'administrator' }, 'u-1');
    assert.equal((await service.authenticate(c.token)).role, 'administrator');
  });

  it('refuses a token whose project was deleted with 410', async () => {
    const c = await service.credential(A);
    await patchRow(service, 'project', projectId(A), { deletedAt: NOW });
    await assert.rejects(service.authenticate(c.token), { status: 410 });
  });
});

describe('revoke', () => {
  it('revokes one of the project’s credentials and records it', async () => {
    const c = await service.credential(A);
    assert.deepEqual(await service.revoke(A, c.id), { ok: true });
    assert.equal((await service.events(A)).at(-1).type, 'credential.revoked');
  });

  it('cannot revoke another project’s credential, which keeps working', async () => {
    const theirs = await service.credential(B);
    await assert.rejects(service.revoke(A, theirs.id), { status: 404, code: 'not_found' });
    assert.equal((await service.authenticate(theirs.token)).key, B);
  });
});

describe('share', () => {
  it('shares a live browser view-only by default, for an hour, bound to that session', async () => {
    await readySession(service, A, 's1');
    const link = await service.share(A, { id: 's1' });
    assert.equal(link.role, 'viewer');
    assert.equal(link.label, 'Shared browser (view)');
    assert.equal(link.expiresAt, NOW + 3600_000);
    assert.equal((await service.authenticate(link.token)).sessionId, 's1');
  });

  it('grants operator control when asked', async () => {
    await readySession(service, A, 's1');
    assert.equal((await service.share(A, { id: 's1', control: true })).role, 'operator');
  });

  it('clamps the lifetime to between one minute and thirty days', async () => {
    await readySession(service, A, 's1');
    assert.equal((await service.share(A, { id: 's1', expiresIn: 1 })).expiresAt, NOW + 60_000);
    assert.equal((await service.share(A, { id: 's1', expiresIn: 1e9 })).expiresAt, NOW + 30 * 86_400_000);
  });

  it('refuses a session that has ended, is unknown, or belongs to another project', async () => {
    await readySession(service, A, 's1');
    await readySession(service, B, 'theirs');
    await service.cancel(A, 's1', { force: true });
    for (const id of ['s1', 'missing', 'theirs'])
      await assert.rejects(service.share(A, { id }), { status: 404, message: 'Ready session not found' });
  });
});

describe('enrollmentCredential', () => {
  it('gives a managed browser a credential that ends with its session', async () => {
    await service.reserve(A, { id: 's1', provider: 'cdp' });
    const c = await service.enrollmentCredential(A, 's1');
    assert.equal(c.role, 'browser');
    assert.equal((await service.authenticate(c.token)).sessionId, 's1');
    await service.complete(A, 's1', 400, {});
    await assert.rejects(service.authenticate(c.token), { status: 401, message: 'Managed browser session has ended' });
  });
});
