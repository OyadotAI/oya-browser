/** Consent appears only above the native page and confirms only the exact main-owned proposal. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  TranslateDialogViewModel,
  confirmationText,
} from '../../../../../../src/renderer/features/toolbar/view-models/translate-dialog-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** One selected tab's proposal, as published by main. */
const tab = (id = 'proposal') =>
  ({
    id: 3,
    translation: {
      state: 'confirming',
      lang: 'fr',
      target: 'en-US',
      confirmation: { id, origin: 'https://example.fr', pages: 20 },
    },
  }) as any;
/** Let the serialized overlay operations settle. */
const settle = () => new Promise(setImmediate);

test('consent waits for the native overlay and sends only the displayed proposal', async () => {
  const overlay = Promise.withResolvers<void>();
  const fake = fakeBridge({ showOverlay: () => overlay.promise });
  const vm = new TranslateDialogViewModel(fake.bridge);
  vm.follow(tab());
  await settle();
  assert.equal(vm.state.confirmation, null);
  vm.confirm();
  assert.deepEqual(fake.called('confirmTranslation'), []);
  overlay.resolve();
  await settle();
  vm.confirm();
  assert.deepEqual(fake.called('confirmTranslation'), [[3, 'proposal']]);
  vm.dispose();
});

test('a tab switch during native overlay setup cannot reopen the old consent', async () => {
  const overlay = Promise.withResolvers<void>();
  const fake = fakeBridge({ showOverlay: () => overlay.promise });
  const vm = new TranslateDialogViewModel(fake.bridge);
  vm.follow(tab());
  await settle();
  vm.follow(undefined);
  overlay.resolve();
  await settle();
  assert.equal(vm.state.confirmation, null);
  assert.deepEqual(fake.called('hideOverlay'), [['translation']]);
  vm.confirm();
  assert.deepEqual(fake.called('confirmTranslation'), []);
  vm.dispose();
});

test('Cancel revokes the proposal without sending confirmation', async () => {
  const fake = fakeBridge();
  const vm = new TranslateDialogViewModel(fake.bridge);
  vm.follow(tab());
  await settle();
  vm.cancel();
  assert.deepEqual(fake.called('showOriginal'), [[3]]);
  assert.deepEqual(fake.called('confirmTranslation'), []);
  vm.dispose();
});

test('failed native overlay setup revokes the hidden proposal', async () => {
  const fake = fakeBridge({ showOverlay: () => Promise.reject(Error('closed')) });
  const vm = new TranslateDialogViewModel(fake.bridge);
  vm.follow(tab());
  await settle();
  assert.equal(vm.state.confirmation, null);
  assert.deepEqual(fake.called('showOriginal'), [[3]]);
  assert.deepEqual(fake.called('hideOverlay'), [['translation']]);
  vm.dispose();
});

test('disclosure states the exact site and continuation bound, or current-page-only consent', () => {
  const t = tab().translation;
  const copy = confirmationText({ tabId: 3, translation: t });
  assert.match(copy.continuation, /20/);
  assert.match(copy.continuation, /https:\/\/example.fr/);
  assert.match(copy.disclosure, /provider/);
  assert.match(copy.disclosure, /Assistant/);
  t.confirmation.pages = 0;
  assert.match(confirmationText({ tabId: 3, translation: t }).continuation, /current page/i);
});

test('confirm failure is visible and a deliberate retry sends the same proposal once', async () => {
  const pending = Promise.withResolvers<void>();
  const fake = fakeBridge({ confirmTranslation: () => pending.promise });
  const vm = new TranslateDialogViewModel(fake.bridge);
  vm.follow(tab());
  await settle();
  vm.confirm();
  vm.confirm();
  assert.equal(fake.called('confirmTranslation').length, 1);
  pending.reject(Error('bridge unavailable'));
  await settle();
  assert.equal(vm.state.action, 'confirm_failed');
  vm.follow(tab());
  assert.equal(vm.state.action, 'confirm_failed', 'same proposal updates retain feedback');
  vm.confirm();
  assert.equal(fake.called('confirmTranslation').length, 2);
  assert.equal(vm.state.action, 'confirm');
  await settle();
  vm.dispose();
});

test('cancellation supersedes pending confirmation and remains retryable after rejection', async () => {
  const confirm = Promise.withResolvers<void>(),
    cancel = Promise.withResolvers<void>();
  const fake = fakeBridge({ confirmTranslation: () => confirm.promise, showOriginal: () => cancel.promise });
  const vm = new TranslateDialogViewModel(fake.bridge);
  vm.follow(tab());
  await settle();
  vm.confirm();
  vm.cancel();
  vm.cancel();
  vm.confirm();
  assert.equal(fake.called('showOriginal').length, 1);
  assert.equal(fake.called('confirmTranslation').length, 1);
  confirm.reject(Error('old confirm'));
  await settle();
  assert.equal(vm.state.action, 'cancel');
  cancel.reject(Error('cancel failed'));
  await settle();
  assert.equal(vm.state.action, 'cancel_failed');
  vm.confirm();
  assert.equal(fake.called('confirmTranslation').length, 1);
  vm.cancel();
  assert.equal(fake.called('showOriginal').length, 2);
  await settle();
  vm.dispose();
});

test('late action rejection cannot affect replacement, withdrawn or disposed proposals', async () => {
  for (const replacement of ['new', 'withdraw', 'dispose']) {
    const pending = Promise.withResolvers<void>();
    const fake = fakeBridge({ confirmTranslation: () => pending.promise });
    const vm = new TranslateDialogViewModel(fake.bridge);
    vm.follow(tab());
    await settle();
    vm.confirm();
    if (replacement === 'dispose') vm.dispose();
    else vm.follow(replacement === 'new' ? tab('replacement') : undefined);
    await settle();
    const state = vm.state;
    pending.reject(Error('old proposal'));
    await settle();
    assert.equal(vm.state, state);
    assert.equal(vm.state.action, null);
    assert.deepEqual(fake.called('showOriginal'), []);
    vm.dispose();
  }
});

test('a stale overlay failure cannot revoke a replacement proposal on the same tab', async () => {
  const overlay = Promise.withResolvers<void>();
  let calls = 0;
  const fake = fakeBridge({ showOverlay: () => (++calls === 1 ? overlay.promise : Promise.resolve()) });
  const vm = new TranslateDialogViewModel(fake.bridge);
  vm.follow(tab());
  await settle();
  vm.follow(tab('replacement'));
  overlay.reject(Error('old overlay'));
  await settle();
  assert.equal(vm.state.confirmation?.translation.confirmation?.id, 'replacement');
  assert.deepEqual(fake.called('showOriginal'), []);
  vm.dispose();
});
