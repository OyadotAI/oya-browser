/** Inbox arrivals are quiet and overlay ownership survives asynchronous races. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { NotificationsViewModel } from '../../../../../src/renderer/features/notifications/view-model.ts';
import { fakeBridge } from '../../support/bridge.ts';
/** Flush promise continuations without time or networking. */
const settle = () => new Promise((resolve) => setImmediate(resolve));
const entry = { id: 'one', message: 'Reminder', source: 'https://example.test', time: 1, read: false };
it('updates the unread badge without opening or hiding the browser', async () => {
  const fake = fakeBridge({ listNotifications: [entry] });
  const vm = new NotificationsViewModel(fake.bridge);
  fake.emit('onNotificationsChanged');
  await settle();
  assert.equal(vm.unread, 1);
  assert.equal(vm.state.open, false);
  assert.equal(fake.called('showOverlay').length, 0);
  vm.dispose();
});
it('ignores stale list responses after newer notifications arrive', async () => {
  const old = Promise.withResolvers();
  let calls = 0;
  const fake = fakeBridge({ listNotifications: () => (++calls === 1 ? old.promise : [entry]) });
  const vm = new NotificationsViewModel(fake.bridge);
  await vm.refresh();
  old.resolve([]);
  await settle();
  assert.deepEqual(vm.state.items, [entry]);
  vm.dispose();
});
it('waits for a pending show before hiding the inbox overlay', async () => {
  const show = Promise.withResolvers();
  const fake = fakeBridge({ listNotifications: [], showOverlay: () => show.promise });
  const vm = new NotificationsViewModel(fake.bridge);
  vm.open();
  await settle();
  vm.close();
  assert.equal(fake.called('hideOverlay').length, 0);
  show.resolve();
  await settle();
  assert.deepEqual(fake.called('hideOverlay'), [['notifications']]);
  vm.dispose();
});
it('clears private text and closes the inbox on profile changes', async () => {
  let entries = [entry];
  const fake = fakeBridge({ listNotifications: () => entries });
  const vm = new NotificationsViewModel(fake.bridge);
  await settle();
  vm.open();
  entries = [];
  fake.emit('onFingerprintChanged', {});
  assert.deepEqual(vm.state.items, []);
  assert.equal(vm.state.open, false);
  await settle();
  vm.dispose();
});
it('marks the displayed ids read and supports individual and full dismissal', async () => {
  const fake = fakeBridge({ listNotifications: [entry] });
  const vm = new NotificationsViewModel(fake.bridge);
  await settle();
  await vm.read();
  await vm.dismiss(entry.id);
  await vm.dismiss();
  assert.deepEqual(
    fake.called('readNotifications'),
    [['one']].map((ids) => [ids]),
  );
  assert.deepEqual(fake.called('dismissNotification'), [['one'], [undefined]]);
  vm.dispose();
});
it('reports IPC failures and removes subscriptions on disposal', async () => {
  const fake = fakeBridge({ listNotifications: () => Promise.reject(new Error('offline')) });
  const vm = new NotificationsViewModel(fake.bridge);
  await settle();
  assert.match(vm.state.error, /Could not load/);
  vm.dispose();
  assert.equal(fake.listeners('onNotificationsChanged'), 0);
  assert.equal(fake.listeners('onFingerprintChanged'), 0);
});
