/** Session inbox retention, privacy, and explicit read/dismiss semantics. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { Notifications } from '../../../../src/main/notifications/index.ts';
import { NOTIFICATION_LIMIT, NOTIFICATION_MESSAGE_LIMIT } from '../../../../src/main/notifications/constants.ts';
/** An isolated inbox with observable invalidation events. */
function fixture() {
  const state = { partition: 'a', changes: 0 };
  const inbox = new Notifications({ partition: () => state.partition, changed: () => state.changes++ });
  return { inbox, state };
}
it('retains literal text and a credential-free origin, newest first', () => {
  const { inbox, state } = fixture();
  inbox.add('<script>not markup</script>', 'https://user:secret@example.test/private?token=secret');
  const [entry] = inbox.list();
  assert.equal(entry.message, '<script>not markup</script>');
  assert.equal(entry.source, 'https://example.test');
  assert.equal(entry.read, false);
  assert.equal(state.changes, 1);
  inbox.add('new', 'data:text/plain,secret');
  assert.equal(inbox.list()[0].source, 'Page alert');
  assert.equal(inbox.list()[0].message, 'new');
});
it('bounds retained messages and entry count', () => {
  const { inbox } = fixture();
  for (let i = 0; i <= NOTIFICATION_LIMIT; i++) inbox.add('x'.repeat(NOTIFICATION_MESSAGE_LIMIT + 1));
  assert.equal(inbox.list().length, NOTIFICATION_LIMIT);
  assert.equal(inbox.list()[0].message.length, NOTIFICATION_MESSAGE_LIMIT);
});
it('marks only displayed ids read and dismisses individually or all', () => {
  const { inbox } = fixture();
  inbox.add('first');
  const id = inbox.list()[0].id;
  inbox.add('second');
  assert.deepEqual(
    inbox.read([id]).map((item) => item.read),
    [false, true],
  );
  assert.equal(inbox.dismiss(id).length, 1);
  assert.deepEqual(inbox.dismiss(), []);
});
it('discards old-profile reminders and never restores them on switching back', () => {
  const { inbox, state } = fixture();
  inbox.add('private a');
  state.partition = 'b';
  assert.deepEqual(inbox.list(), []);
  inbox.add('private b');
  state.partition = 'a';
  assert.deepEqual(inbox.list(), []);
});
it('returns copies so readers cannot mutate retained state', () => {
  const { inbox } = fixture();
  inbox.add('original');
  inbox.list()[0].message = 'changed';
  assert.equal(inbox.list()[0].message, 'original');
});
