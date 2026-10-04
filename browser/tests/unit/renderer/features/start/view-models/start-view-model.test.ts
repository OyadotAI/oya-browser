/**
 * Unit tests for the Oya start page: it shows while the active tab is on it,
 * tells the shell (body.on-home), replays its arrival, and hands a task to
 * Ask unless it is empty or Ask is busy.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { StartViewModel } from '../../../../../../src/renderer/features/start/view-models/start-view-model.ts';
import { ShellViewModel } from '../../../../../../src/renderer/app/shell-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** A start page over a fake bridge, a real shell, and an Ask that records tasks. */
function start({ busy = false } = {}) {
  const fake = fakeBridge();
  const shell = new ShellViewModel(fake.bridge);
  const asked: string[] = [];
  const asker = { ask: (text: string) => void asked.push(text), busy: () => busy };
  const vm = new StartViewModel({ bridge: fake.bridge, shell, asker });
  /** Reports one active tab, on the start page or on a site. */
  const activeTab = (home: boolean) =>
    fake.emit('onTabsUpdated', [{ id: 1, active: true, home, url: home ? '' : 'https://a.test/' }]);
  return { fake, shell, asked, vm, activeTab };
}

describe('StartViewModel', () => {
  it('shows exactly while the active tab is on the start page, and tells the shell', () => {
    const { shell, vm, activeTab } = start();
    activeTab(true);
    assert.equal(vm.state.home, true);
    assert.equal(shell.state.onHome, true);
    activeTab(false);
    assert.equal(vm.state.home, false);
    assert.equal(shell.state.onHome, false);
  });

  it('plays its arrival each time it comes back, not on every tab update', () => {
    const { vm, activeTab } = start();
    activeTab(true);
    activeTab(true);
    assert.equal(vm.state.arrivals, 1);
    activeTab(false);
    activeTab(true);
    assert.equal(vm.state.arrivals, 2);
  });

  it('hands a typed task to Ask, trimmed, so the box can clear', () => {
    const { asked, vm } = start();
    assert.equal(vm.ask('  Find Jordans on Amazon \n'), true);
    assert.deepEqual(asked, ['Find Jordans on Amazon']);
  });

  it('ignores an empty box', () => {
    const { asked, vm } = start();
    assert.equal(vm.ask('   '), false);
    assert.deepEqual(asked, []);
  });

  it('keeps the task while Ask is busy with another', () => {
    const { asked, vm } = start({ busy: true });
    assert.equal(vm.ask('a task'), false);
    assert.deepEqual(asked, []);
  });

  it('stops following the tab list on dispose', () => {
    const { fake, vm } = start();
    vm.dispose();
    assert.equal(fake.listeners('onTabsUpdated'), 0);
  });
});
