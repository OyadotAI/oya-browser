/**
 * Unit tests for the find bar's ViewModel: it follows the main process's state,
 * searches as the person types, steps on Enter and Shift+Enter (not while an
 * input method composes, nor with an empty field), closes on Esc, and asks the
 * field to take focus on Cmd/Ctrl+F.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import {
  FindBarViewModel,
  findCount,
} from '../../../../../../src/renderer/features/toolbar/view-models/find-bar-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** A keydown as the field hands it over; records whether it was kept from the field. */
const press = (key: string, extra = {}) => ({
  key,
  shiftKey: false,
  isComposing: false,
  prevented: false,
  preventDefault() {
    this.prevented = true;
  },
  ...extra,
});

it('says nothing before a search, "No results" for none, and "N of M" otherwise', () => {
  assert.equal(findCount({ open: true, active: 0, total: null }), '');
  assert.equal(findCount({ open: true, active: 0, total: 0 }), 'No results');
  assert.equal(findCount({ open: true, active: 3, total: 17 }), '3 of 17');
});

it('follows the main process, and reopens with an empty field', () => {
  const fake = fakeBridge();
  const vm = new FindBarViewModel(fake.bridge);
  fake.emit('onFindBar', { open: true, active: 0, total: null });
  vm.type('cat');
  fake.emit('onFindBar', { open: true, active: 2, total: 5 });
  assert.deepEqual([vm.state.active, vm.state.total, vm.state.query], [2, 5, 'cat']);
  fake.emit('onFindBar', { open: false, active: 0, total: null });
  assert.deepEqual([vm.state.open, vm.state.query], [false, '']);
  vm.dispose();
});

it('searches as the person types', () => {
  const fake = fakeBridge();
  const vm = new FindBarViewModel(fake.bridge);
  vm.type('c');
  vm.type('ca');
  assert.deepEqual(fake.called('findInPage'), [['c'], ['ca']]);
  vm.dispose();
});

it('steps forward on Enter and back on Shift+Enter', () => {
  const fake = fakeBridge();
  const vm = new FindBarViewModel(fake.bridge);
  vm.type('cat');
  const enter = press('Enter');
  vm.key(enter);
  vm.key(press('Enter', { shiftKey: true }));
  assert.deepEqual(fake.called('findStep'), [[true], [false]]);
  assert.equal(enter.prevented, true);
  vm.dispose();
});

it('does not step while an input method composes, or with an empty field', () => {
  const fake = fakeBridge();
  const vm = new FindBarViewModel(fake.bridge);
  vm.key(press('Enter'));
  vm.type('猫');
  vm.key(press('Enter', { isComposing: true }));
  assert.deepEqual(fake.called('findStep'), []);
  vm.dispose();
});

it('closes on Esc', () => {
  const fake = fakeBridge();
  const vm = new FindBarViewModel(fake.bridge);
  vm.key(press('Escape'));
  assert.equal(fake.called('closeFind').length, 1);
  vm.dispose();
});

it('asks the field to take focus on each Cmd/Ctrl+F, and ignores other commands', () => {
  const fake = fakeBridge();
  const vm = new FindBarViewModel(fake.bridge);
  fake.emit('onShellCommand', 'find');
  fake.emit('onShellCommand', 'address');
  fake.emit('onShellCommand', 'find');
  assert.equal(vm.state.focusRequest, 2);
  vm.dispose();
  assert.equal(fake.listeners('onFindBar'), 0);
});
