/**
 * Unit tests for the ViewModel base: a change replaces the snapshot and tells
 * subscribers, an unchanged patch tells no one, and dispose stops everything.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ViewModel } from '../../../../src/renderer/core/view-model.ts';

/** The counter's state. */
interface Count {
  /** How many. */
  count: number;
  /** What it counts. */
  label: string;
}

/** A counter, to drive the base class. */
class Counter extends ViewModel<Count> {
  /** Starts at zero. */
  constructor() {
    super({ count: 0, label: 'n' });
  }
  /** Adds one. */
  increment() {
    this.set({ count: this.state.count + 1 });
  }
  /** Sets the label. */
  label(label: string) {
    this.set({ label });
  }
  /** Keeps a cleanup. */
  hold(cleanup: () => void) {
    this.own(cleanup);
  }
}

describe('ViewModel', () => {
  it('replaces the snapshot on a change and tells subscribers', () => {
    const vm = new Counter();
    const before = vm.getSnapshot();
    let told = 0;
    vm.subscribe(() => told++);
    vm.increment();
    assert.equal(vm.state.count, 1);
    assert.notEqual(vm.getSnapshot(), before);
    assert.equal(before.count, 0);
    assert.equal(told, 1);
  });

  it('tells no one when a patch changes nothing', () => {
    const vm = new Counter();
    let told = 0;
    vm.subscribe(() => told++);
    vm.label('n');
    assert.equal(told, 0);
  });

  it('stops telling a subscriber that unsubscribed', () => {
    const vm = new Counter();
    let told = 0;
    const stop = vm.subscribe(() => told++);
    stop();
    vm.increment();
    assert.equal(told, 0);
  });

  it('runs what it owns once on dispose, and drops its subscribers', () => {
    const vm = new Counter();
    let cleaned = 0;
    let told = 0;
    vm.hold(() => cleaned++);
    vm.subscribe(() => told++);
    vm.dispose();
    vm.dispose();
    vm.increment();
    assert.equal(cleaned, 1);
    assert.equal(told, 0);
  });
});
