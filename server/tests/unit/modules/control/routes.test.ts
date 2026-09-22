/**
 * Unit tests for the /control REST API, called in-process: the overview by
 * role, sessions scoped to the caller's project, administrator-only settings,
 * members, credentials, share links, tickets and webhooks, the event cursor,
 * and coded JSON errors. One project can never read or change another's rows.
 */
import { describe, it, before, after, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-control-routes-');
const { controlRouter } = await import('../../../../src/modules/control/routes.ts');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');
const { allowKey, callRoute } = await import('../../support/agent.ts');
const { connectBrowser, disconnectBrowser } = await import('../../support/fakes.ts');
const { readySession } = await import('../../support/control.ts');

const A = 'routes-key-a',
  B = 'routes-key-b';
let forget = [],
  n = 0,
  id;
before(() => (forget = [allowKey(A), allowKey(B)]));
after(() => forget.forEach((f) => f()));
beforeEach(async () => {
  id = `rt-${n++}`;
  await readySession(control(), A, id);
});

/** Calls the router as `key`. */
const call = (method, url, body?, key = A) => callRoute(controlRouter, { method, url, body, key });

describe('GET /control', () => {
  it('shows an administrator the whole overview', async () => {
    const res = await call('GET', '/');
    assert.equal(res.body.project.id, projectId(A));
    assert.ok(Array.isArray(res.body.credentials));
  });

  it('hides credentials, webhooks and deliveries from anyone else', async () => {
    const viewer = await control().credential(A, { role: 'viewer' });
    const res = await call('GET', '/', undefined, viewer.token);
    assert.equal(res.status, 200);
    assert.deepEqual([res.body.credentials, res.body.webhooks, res.body.deliveries], [undefined, undefined, undefined]);
  });

  it('refuses a caller with no credential', async () => {
    assert.equal((await callRoute(controlRouter, { url: '/' })).status, 401);
  });
});

describe('control error handler', () => {
  it('answers a thrown plain error as a 500 with a reference, never as control unavailable', async () => {
    const original = control().sessions;
    control().sessions = async () => {
      throw new Error('SQLITE_BUSY: database is locked');
    };
    const logged = mock.method(console, 'error', () => {});
    try {
      const res = await call('GET', '/sessions');
      assert.deepEqual([res.status, res.body.code], [500, 'internal_error']);
      assert.ok(!res.body.error.includes('SQLITE'));
      assert.match(res.body.ref, /^[0-9a-f]{8}$/);
    } finally {
      control().sessions = original;
      logged.mock.restore();
    }
  });
});

describe('sessions routes', () => {
  it('lists only the caller’s sessions', async () => {
    await readySession(control(), B, `${id}-b`);
    const ids = (await call('GET', '/sessions')).body.map((x) => x.id);
    assert.ok(ids.includes(id));
    assert.equal(ids.includes(`${id}-b`), false);
  });

  it('answers another project’s session with a coded 404', async () => {
    const res = await call('GET', `/sessions/${id}`, undefined, B);
    assert.deepEqual([res.status, res.body], [404, { error: 'Session not found', code: 'not_found' }]);
  });

  it('cancels the caller’s own session', async () => {
    assert.equal((await call('POST', `/sessions/${id}/cancel`)).body.state, 'cleanup_pending');
  });

  it('issues a 60-second ticket for a ready session only', async () => {
    const res = await call('POST', `/sessions/${id}/ticket`);
    assert.equal(res.body.expiresIn, 60);
    assert.equal(await control().redeem(res.body.ticket, id), A);
    await control().cancel(A, id);
    assert.equal((await call('POST', `/sessions/${id}/ticket`)).status, 404);
  });

  it('moves control and tells the connected browser', async () => {
    const ws = connectBrowser(id, A);
    try {
      const res = await call('POST', `/sessions/${id}/control`, { action: 'acquire' });
      assert.equal(res.body.mode, 'human');
      assert.equal(ws.ofType('control_mode')[0].mode, 'human');
    } finally {
      disconnectBrowser(id);
    }
  });

  it('shares a live browser as a 201 credential bound to it', async () => {
    const res = await call('POST', `/sessions/${id}/share`, { control: true, expiresIn: 120 });
    assert.equal(res.status, 201);
    assert.equal(res.body.role, 'operator');
    assert.equal(res.body.sessionId, id);
  });
});

describe('administrator routes', () => {
  it('refuses a non-administrator', async () => {
    const operator = await control().credential(A, { role: 'operator' });
    const res = await call('PATCH', '/project', { auditDays: 5 }, operator.token);
    assert.equal(res.status, 403);
  });

  it('updates settings', async () => {
    assert.equal((await call('PATCH', '/project', { auditDays: 5 })).body.settings.auditDays, 5);
  });

  it('invites an operator by default and lists members', async () => {
    const invite = await call('POST', '/members/invite', {});
    assert.equal(invite.status, 201);
    assert.ok(invite.body.code);
    const members = await call('GET', '/members');
    assert.deepEqual(Object.keys(members.body).sort(), ['members', 'owner']);
    const gone = await call('DELETE', '/members/nobody');
    assert.deepEqual([gone.status, gone.body.code], [404, 'not_found']);
  });

  it('issues and revokes credentials', async () => {
    const created = await call('POST', '/credentials', { role: 'viewer' });
    assert.equal(created.status, 201);
    assert.equal((await call('DELETE', `/credentials/${created.body.id}`)).body.ok, true);
    assert.equal((await call('DELETE', `/credentials/${created.body.id}`, undefined, B)).status, 404);
  });
});

describe('GET /control/events', () => {
  it('returns events after the cursor, and the new cursor', async () => {
    const all = await call('GET', '/events');
    assert.equal(all.body.cursor, all.body.events.at(-1).id);
    const later = await call('GET', `/events?after=${all.body.cursor}`);
    assert.deepEqual([later.body.events, later.body.cursor], [[], all.body.cursor]);
  });

  it('refuses a malformed cursor', async () => {
    for (const after of ['-1', 'abc', '1.5'])
      assert.deepEqual((await call('GET', `/events?after=${after}`)).status, 400);
  });
});

describe('webhook routes', () => {
  it('refuses a webhook URL that is not public HTTPS', async () => {
    for (const url of ['http://93.184.216.34/hook', 'https://127.0.0.1/hook'])
      assert.equal((await call('PUT', '/webhook', { url })).status, 400);
  });

  it('saves, shows and disables the project webhook', async () => {
    const saved = await call('PUT', '/webhook', { url: 'https://93.184.216.34/hook', types: ['session.ready'] });
    assert.ok(saved.body.secret);
    assert.equal((await call('GET', '/webhook')).body.hook.enabled, true);
    await call('DELETE', '/webhook');
    assert.equal((await call('GET', '/webhook')).body.hook.enabled, false);
  });

  it('adds a webhook through the legacy route', async () => {
    const res = await call('POST', '/webhooks', { url: 'https://93.184.216.34/hook' });
    assert.equal(res.status, 201);
  });

  it('never disables another project’s webhook or replays its deliveries', async () => {
    await control().webhook(B, { url: 'https://93.184.216.34/b', types: ['run.failed'] });
    await control().emit(B, 'run.failed');
    const [delivery] = await control().store.list('delivery', { project: projectId(B) });
    assert.equal((await call('DELETE', `/webhooks/hook:${projectId(B)}`)).status, 404);
    assert.equal((await call('POST', `/deliveries/${delivery.id}/replay`)).status, 404);
    assert.equal((await control().store.get('webhook', `hook:${projectId(B)}`)).enabled, true);
  });

  it('disables its own webhook and queues its own delivery again', async () => {
    await control().webhook(A, { url: 'https://93.184.216.34/a', types: ['run.failed'] });
    await control().emit(A, 'run.failed');
    const [delivery] = await control().store.list('delivery', { project: projectId(A) });
    await control().store.transact(async (tx) => {
      Object.assign(await tx.get('delivery', delivery.id), { state: 'failed', attempts: 9 });
    });
    assert.equal((await call('POST', `/deliveries/${delivery.id}/replay`)).body.ok, true);
    const replayed = await control().store.get('delivery', delivery.id);
    assert.deepEqual([replayed.state, replayed.attempts], ['pending', 0]);
    assert.equal((await call('DELETE', `/webhooks/hook:${projectId(A)}`)).body.ok, true);
  });
});
