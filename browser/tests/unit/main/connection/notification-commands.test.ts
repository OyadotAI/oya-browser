/** Notification tools use the real dispatcher, shared inbox and normal result envelopes. */
import { it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { CommandRunner } from '../../../../src/main/connection/commands.ts';
import { Notifications } from '../../../../src/main/notifications/index.ts';
import { mainCtx } from '../../support/main-ctx.cjs';
let ctx, profile, changes;
beforeEach(() => {
  ctx = mainCtx({ commands: CommandRunner });
  profile = 'a';
  changes = 0;
  ctx.notifications = new Notifications({ partition: () => profile, changed: () => changes++ });
});
/** Invoke a command without an active page or real network. */
async function run(action, params = {}) {
  await ctx.commands.handleCommand({ id: 'inbox', action, params });
  return ctx.socket.ofType('cmd_result').at(-1);
}
it('lists newest-first with pagination, unread count, and no read side effects', async () => {
  ctx.notifications.add('first');
  ctx.notifications.add('second');
  const result = await run('list_notifications', { limit: 1 });
  assert.equal(result.ok, true);
  assert.equal(result.data.entries[0].message, 'second');
  assert.equal(result.data.next_offset, 1);
  assert.equal(result.data.total, 2);
  assert.equal(result.data.unread_count, 2);
  assert.equal(result.data.session_only, true);
  assert.equal((await run('list_notifications', { offset: 1 })).data.entries[0].message, 'first');
  assert.equal(changes, 2);
});
it('marks only explicit ids read, updates the toolbar, and is retry-safe', async () => {
  ctx.notifications.add('first');
  const id = ctx.notifications.list()[0].id;
  ctx.notifications.add('later');
  assert.equal((await run('mark_notifications_read', { ids: [id, id] })).data.marked_read, 1);
  assert.equal(changes, 3);
  assert.equal((await run('mark_notifications_read', { ids: [id] })).data.marked_read, 0);
  assert.equal((await run('list_notifications', { unread_only: true })).data.total, 1);
});
it('never turns missing ids or missing confirmation into a clear-all operation', async () => {
  ctx.notifications.add('keep');
  const id = ctx.notifications.list()[0].id;
  for (const params of [{}, { confirm: true }, { notification_id: id }, { notification_id: id, confirm: 'true' }])
    assert.equal((await run('dismiss_notification', params)).ok, false);
  assert.equal(ctx.notifications.list().length, 1);
  assert.equal((await run('dismiss_notification', { notification_id: id, confirm: true })).data.removed, true);
  assert.equal((await run('dismiss_notification', { notification_id: id, confirm: true })).data.removed, false);
});
it('validates all ids before changing any read state', async () => {
  ctx.notifications.add('keep unread');
  const id = ctx.notifications.list()[0].id;
  for (const ids of [undefined, [], 'all', [id, null], [' '], ['x'.repeat(129)], Array(101).fill(id)])
    assert.equal((await run('mark_notifications_read', { ids })).ok, false);
  assert.equal(ctx.notifications.list()[0].read, false);
});
it('rejects invalid pagination and filters at the desktop boundary', async () => {
  for (const params of [
    { limit: 0 },
    { limit: 51 },
    { limit: 1.5 },
    { limit: '1' },
    { offset: -1 },
    { unread_only: 'true' },
  ])
    assert.equal((await run('list_notifications', params)).ok, false);
  assert.equal((await run('list_notifications', { offset: 10 })).data.next_offset, null);
});
it('cannot read or dismiss another profile’s notifications', async () => {
  ctx.notifications.add('private a');
  const id = ctx.notifications.list()[0].id;
  profile = 'b';
  assert.equal((await run('list_notifications')).data.total, 0);
  assert.equal((await run('dismiss_notification', { notification_id: id, confirm: true })).data.removed, false);
});
